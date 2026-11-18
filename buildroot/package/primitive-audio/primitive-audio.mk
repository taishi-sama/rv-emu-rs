################################################################################
#
# primitive-audio
#
################################################################################

PRIMITIVE_AUDIO_VERSION = 1.0
PRIMITIVE_AUDIO_SITE = $(BR2_EXTERNAL_RVEMURS_PATH)/package/primitive-audio
PRIMITIVE_AUDIO_SITE_METHOD = local
PRIMITIVE_AUDIO_LICENSE = GPL-2.0
PRIMITIVE_AUDIO_MODULE_SUBDIRS = src

$(eval $(kernel-module))
$(eval $(generic-package))
