plugins {
    id("com.android.application")
}

// Yerel tun -> SOCKS köprüsü (hev-socks5-tunnel, MIT). Kaynak depoya girmez, CI sabit sürümü
// android/hev-socks5-tunnel klasörüne indirir. Klasör yoksa yalnızca Kotlin kısmı derlenir.
val hevDir = rootProject.file("hev-socks5-tunnel")
val withNative = hevDir.resolve("Android.mk").exists()

android {
    namespace = "io.github.technodiance23.gdpi"
    compileSdk = 36

    defaultConfig {
        applicationId = "io.github.technodiance23.gdpi"
        minSdk = 24
        targetSdk = 36
        versionCode = (findProperty("gdpi.versionCode") as String?)?.toInt() ?: 1
        versionName = (findProperty("gdpi.versionName") as String?) ?: "dev"

        if (withNative) {
            ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64") }
            externalNativeBuild {
                ndkBuild {
                    arguments += listOf(
                        "APP_CFLAGS+=-DPKGNAME=io/github/technodiance23/gdpi -ffile-prefix-map=${rootDir}=.",
                        "APP_LDFLAGS+=-Wl,--build-id=none",
                        "APP_SUPPORT_FLEXIBLE_PAGE_SIZES=true",
                    )
                }
            }
        }
    }

    if (withNative) {
        externalNativeBuild {
            ndkBuild { path = hevDir.resolve("Android.mk") }
        }
    }

    // İmza bilgileri yalnızca ortam değişkenlerinden okunur (GitHub Actions secret'ları).
    val ks = System.getenv("GDPI_KEYSTORE_FILE")
    signingConfigs {
        if (ks != null && file(ks).exists()) {
            create("release") {
                storeFile = file(ks)
                storePassword = System.getenv("GDPI_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("GDPI_KEY_ALIAS")
                keyPassword = System.getenv("GDPI_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.findByName("release") ?: signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    packaging { jniLibs { useLegacyPackaging = false } }
}

dependencies {
    testImplementation("junit:junit:4.13.2")
}
