/*
 * Root olmadan sahte paket.
 *
 * Fikir (ByeDPI'daki gibi): veriyi kendi belleğimizden vmsplice + splice ile sokete veriyoruz,
 * böylece çekirdek baytları kopyalamıyor, bizim sayfamızı gösteriyor. Önce sayfaya sahte içeriği
 * yazıp düşük TTL ile gönderiyoruz; paket DPI'dan geçip sunucuya varmadan ölüyor. Sonra aynı
 * sayfanın üstüne gerçek içeriği yazıyoruz. Onay gelmeyince çekirdek segmenti yeniden yolluyor,
 * bu sefer gerçek baytlarla. Sunucu hiçbir zaman sahte veriyi görmüyor.
 */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <jni.h>
#include <netinet/in.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/socket.h>
#include <sys/uio.h>
#include <unistd.h>

static int ttl_opt(int v6, int *level, int *name)
{
    *level = v6 ? IPPROTO_IPV6 : IPPROTO_IP;
    *name = v6 ? IPV6_UNICAST_HOPS : IP_TTL;
    return 0;
}

JNIEXPORT jint JNICALL
Java_io_github_technodiance23_gdpi_Fake_send(JNIEnv *env, jclass cls, jint fd,
                                             jbyteArray jfake, jbyteArray jreal,
                                             jint ttl, jboolean v6)
{
    (void)cls;
    jsize n = (*env)->GetArrayLength(env, jfake);
    if (n <= 0 || n != (*env)->GetArrayLength(env, jreal))
        return -EINVAL;

    long ps = sysconf(_SC_PAGESIZE);
    size_t size = ((size_t)n + ps - 1) / ps * ps;
    unsigned char *page = mmap(NULL, size, PROT_READ | PROT_WRITE,
                               MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (page == MAP_FAILED)
        return -errno;

    int ret = 0, p[2] = { -1, -1 };
    int level, name, old = 64;
    socklen_t ol = sizeof(old);
    ttl_opt(v6, &level, &name);
    getsockopt(fd, level, name, &old, &ol);

    if (pipe(p) < 0) { ret = -errno; goto out; }
    (*env)->GetByteArrayRegion(env, jfake, 0, n, (jbyte *)page);

    if (setsockopt(fd, level, name, &ttl, sizeof(ttl)) < 0) { ret = -errno; goto out; }

    struct iovec v = { page, (size_t)n };
    ssize_t w = vmsplice(p[1], &v, 1, SPLICE_F_GIFT);
    if (w != n) { ret = w < 0 ? -errno : -EIO; goto restore; }
    ssize_t s = splice(p[0], NULL, fd, NULL, (size_t)n, 0);
    if (s != n) { ret = s < 0 ? -errno : -EIO; goto restore; }

    usleep(5000);                      /* sahte segment kesin çıksın */
    (*env)->GetByteArrayRegion(env, jreal, 0, n, (jbyte *)page);
    ret = n;

restore:
    setsockopt(fd, level, name, &old, sizeof(old));
out:
    if (p[0] >= 0) close(p[0]);
    if (p[1] >= 0) close(p[1]);
    munmap(page, size);
    return ret;
}
