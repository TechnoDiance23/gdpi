# hev-socks5-tunnel'ı ve GDPI'ın kendi küçük yerel parçasını (sahte paket) birlikte derler.
GDPI_NATIVE := $(call my-dir)
include $(GDPI_NATIVE)/../hev-socks5-tunnel/Android.mk

include $(CLEAR_VARS)
LOCAL_PATH := $(GDPI_NATIVE)
LOCAL_MODULE := gdpi-fake
LOCAL_SRC_FILES := fake.c
LOCAL_CFLAGS += -Wall -Werror
LOCAL_LDFLAGS += -Wl,-z,max-page-size=16384 -Wl,-z,common-page-size=16384
include $(BUILD_SHARED_LIBRARY)
