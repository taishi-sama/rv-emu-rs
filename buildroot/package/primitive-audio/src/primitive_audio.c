// SPDX-License-Identifier: GPL-2.0
/*
 * ALSA PCM driver for the rv-emu-rs "primitive audio" device.
 *
 * The device has two 32-bit MMIO registers in an 8-byte region:
 *   - offset 0, write halfword: enqueue one signed 16-bit sample
 *   - offset 0, read word:      number of samples still queued
 *   - offset 4, read/write word: current sample rate in Hz
 *
 * Samples are interleaved stereo S16_LE, one halfword per channel sample.  The
 * driver advertises the rate range the device accepts, stores the rate the
 * application chooses in the register, and paces writes from an hrtimer, one
 * ALSA period at a time.
 */

#include <linux/atomic.h>
#include <linux/hrtimer.h>
#include <linux/io.h>
#include <linux/ktime.h>
#include <linux/math64.h>
#include <linux/module.h>
#include <linux/of.h>
#include <linux/platform_device.h>
#include <linux/string.h>
#include <linux/uio.h>
#include <sound/core.h>
#include <sound/initval.h>
#include <sound/pcm.h>

#define DRV_NAME		"primitive_audio"
#define PA_RATE_MIN		8000
#define PA_RATE_MAX		192000
#define PA_SAMPLE_RATE_OFFSET	4
#define PA_CHANNELS		2
#define PA_SAMPLE_BYTES		2

struct primitive_audio {
	struct snd_card *card;
	struct snd_pcm *pcm;
	struct snd_pcm_substream *substream;
	void __iomem *base;
	struct hrtimer timer;
	ktime_t period_time;
	atomic_t running;
	unsigned int hw_ptr;	/* next frame in the ALSA buffer */
};

static const struct snd_pcm_hardware pa_hw = {
	.info		= SNDRV_PCM_INFO_INTERLEAVED |
			  SNDRV_PCM_INFO_BLOCK_TRANSFER,
	.formats	= SNDRV_PCM_FMTBIT_S16_LE,
	.rates		= SNDRV_PCM_RATE_CONTINUOUS,
	.rate_min	= PA_RATE_MIN,
	.rate_max	= PA_RATE_MAX,
	.channels_min	= PA_CHANNELS,
	.channels_max	= PA_CHANNELS,
	.buffer_bytes_max = 256 * 1024,
	.period_bytes_min = 256,
	.period_bytes_max = 64 * 1024,
	.periods_min	= 2,
	.periods_max	= 16,
};

static void pa_write_period(struct primitive_audio *pa)
{
	struct snd_pcm_runtime *rt = pa->substream->runtime;
	unsigned int frame_bytes = rt->channels * PA_SAMPLE_BYTES;
	unsigned int pos = pa->hw_ptr;
	unsigned int i;

	for (i = 0; i < rt->period_size; i++) {
		s16 *frame = (s16 *)(rt->dma_area + pos * frame_bytes);
		unsigned int ch;

		for (ch = 0; ch < rt->channels; ch++)
			writew(frame[ch], pa->base);
		if (++pos >= rt->buffer_size)
			pos = 0;
	}
	pa->hw_ptr = pos;
}

static enum hrtimer_restart pa_timer(struct hrtimer *timer)
{
	struct primitive_audio *pa =
		container_of(timer, struct primitive_audio, timer);

	if (!atomic_read(&pa->running))
		return HRTIMER_NORESTART;

	pa_write_period(pa);
	snd_pcm_period_elapsed(pa->substream);

	if (!atomic_read(&pa->running))
		return HRTIMER_NORESTART;

	hrtimer_forward_now(timer, pa->period_time);
	return HRTIMER_RESTART;
}

static int pa_open(struct snd_pcm_substream *substream)
{
	struct primitive_audio *pa = snd_pcm_substream_chip(substream);
	struct snd_pcm_runtime *runtime = substream->runtime;

	runtime->hw = pa_hw;
	pa->substream = substream;

	return snd_pcm_hw_constraint_integer(runtime, SNDRV_PCM_HW_PARAM_PERIODS);
}

static int pa_close(struct snd_pcm_substream *substream)
{
	struct primitive_audio *pa = snd_pcm_substream_chip(substream);

	atomic_set(&pa->running, 0);
	hrtimer_cancel(&pa->timer);
	pa->substream = NULL;
	return 0;
}

static int pa_prepare(struct snd_pcm_substream *substream)
{
	struct primitive_audio *pa = snd_pcm_substream_chip(substream);
	struct snd_pcm_runtime *runtime = substream->runtime;

	atomic_set(&pa->running, 0);
	pa->hw_ptr = 0;
	if (runtime->dma_area)
		memset(runtime->dma_area, 0, runtime->dma_bytes);
	return 0;
}

static int pa_hw_params(struct snd_pcm_substream *substream,
			struct snd_pcm_hw_params *params)
{
	struct primitive_audio *pa = snd_pcm_substream_chip(substream);

	/* The register is the device's playback clock; the host audio source
	 * follows it when the application changes the rate.
	 */
	writel(params_rate(params), pa->base + PA_SAMPLE_RATE_OFFSET);
	return 0;
}

static int pa_trigger(struct snd_pcm_substream *substream, int cmd)
{
	struct primitive_audio *pa = snd_pcm_substream_chip(substream);
	struct snd_pcm_runtime *runtime = substream->runtime;

	switch (cmd) {
	case SNDRV_PCM_TRIGGER_START:
	case SNDRV_PCM_TRIGGER_RESUME:
		/* div_u64(), because a plain 64/32 division would pull in
		 * libgcc's __udivdi3, which the kernel does not export.
		 */
		pa->period_time = ns_to_ktime(div_u64((u64)runtime->period_size *
						      NSEC_PER_SEC,
						      runtime->rate));
		atomic_set(&pa->running, 1);
		hrtimer_start(&pa->timer, pa->period_time, HRTIMER_MODE_REL_SOFT);
		return 0;
	case SNDRV_PCM_TRIGGER_STOP:
	case SNDRV_PCM_TRIGGER_SUSPEND:
		atomic_set(&pa->running, 0);
		return 0;
	default:
		return -EINVAL;
	}
}

static int pa_sync_stop(struct snd_pcm_substream *substream)
{
	struct primitive_audio *pa = snd_pcm_substream_chip(substream);

	hrtimer_cancel(&pa->timer);
	return 0;
}

static snd_pcm_uframes_t pa_pointer(struct snd_pcm_substream *substream)
{
	struct primitive_audio *pa = snd_pcm_substream_chip(substream);

	return pa->hw_ptr;
}

static int pa_copy(struct snd_pcm_substream *substream, int channel,
		   unsigned long pos, struct iov_iter *iter,
		   unsigned long bytes)
{
	struct snd_pcm_runtime *runtime = substream->runtime;

	if (!copy_from_iter_full(runtime->dma_area + pos, bytes, iter))
		return -EFAULT;
	return 0;
}

static int pa_fill_silence(struct snd_pcm_substream *substream, int channel,
			   unsigned long pos, unsigned long bytes)
{
	memset(substream->runtime->dma_area + pos, 0, bytes);
	return 0;
}

static const struct snd_pcm_ops pa_ops = {
	.open		= pa_open,
	.close		= pa_close,
	.prepare	= pa_prepare,
	.hw_params	= pa_hw_params,
	.trigger	= pa_trigger,
	.sync_stop	= pa_sync_stop,
	.pointer	= pa_pointer,
	.copy		= pa_copy,
	.fill_silence	= pa_fill_silence,
};

static int pa_probe(struct platform_device *pdev)
{
	struct snd_card *card;
	struct primitive_audio *pa;
	struct resource *res;
	int err;

	err = snd_devm_card_new(&pdev->dev, SNDRV_DEFAULT_IDX1,
				SNDRV_DEFAULT_STR1, THIS_MODULE, sizeof(*pa),
				&card);
	if (err < 0)
		return err;

	pa = card->private_data;
	pa->card = card;
	atomic_set(&pa->running, 0);

	res = platform_get_resource(pdev, IORESOURCE_MEM, 0);
	pa->base = devm_ioremap_resource(&pdev->dev, res);
	if (IS_ERR(pa->base))
		return PTR_ERR(pa->base);

	hrtimer_setup(&pa->timer, pa_timer, CLOCK_MONOTONIC,
		      HRTIMER_MODE_REL_SOFT);

	err = snd_pcm_new(card, "primitive-audio", 0, 1, 0, &pa->pcm);
	if (err < 0)
		return err;

	snd_pcm_set_ops(pa->pcm, SNDRV_PCM_STREAM_PLAYBACK, &pa_ops);
	pa->pcm->private_data = pa;
	strscpy(pa->pcm->name, "primitive-audio");

	err = snd_pcm_set_managed_buffer_all(pa->pcm, SNDRV_DMA_TYPE_VMALLOC,
					     NULL, 0, 256 * 1024);
	if (err < 0)
		return err;

	strscpy(card->driver, "primitive-audio");
	strscpy(card->shortname, "rv-emu-rs audio");
	strscpy(card->longname, "rv-emu-rs primitive audio");

	err = snd_card_register(card);
	if (err < 0)
		return err;

	platform_set_drvdata(pdev, card);
	dev_info(&pdev->dev, "ALSA PCM: %u Hz, %d channels, S16_LE\n",
		 readl(pa->base + PA_SAMPLE_RATE_OFFSET), PA_CHANNELS);
	return 0;
}

static const struct of_device_id pa_of_match[] = {
	{ .compatible = "rv-emu-rs,primitive-audio" },
	{ }
};
MODULE_DEVICE_TABLE(of, pa_of_match);

static struct platform_driver pa_driver = {
	.probe	= pa_probe,
	.driver	= {
		.name		= DRV_NAME,
		.of_match_table	= pa_of_match,
	},
};
module_platform_driver(pa_driver);

MODULE_AUTHOR("rv-emu-rs");
MODULE_DESCRIPTION("ALSA driver for the rv-emu-rs primitive audio device");
MODULE_LICENSE("GPL");
