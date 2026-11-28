#!/bin/sh
# Cross-build the pinned upstream packages into an initrd staging tree.
set -eu
repo=$1
work=$2
jobs=$3
port="$repo/ports/x11"
mkdir -p "$work"
cd "$work"
work=$PWD
exec 9> "$work/.lock"
flock 9
prefix="$work/stage"
mkdir -p downloads source objects "$prefix"
export CC="$repo/tools/lumen-clang" AR=ar RANLIB=ranlib
export CFLAGS="-O2 -ffunction-sections -fdata-sections -D__lumen__ -I$prefix/include"
export LDFLAGS="-L$prefix/lib"
export PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig:$prefix/share/pkgconfig"
export PKG_CONFIG_PATH=
export xorg_cv_malloc0_returns_null=no ac_cv_func_malloc_0_nonnull=yes ac_cv_func_realloc_0_nonnull=yes

valid_hash() {
    printf '%s  %s\n' "$2" "$1" | sha256sum --check --status
}

while read -r package checksum url; do
    archive="downloads/$package.tar"
    if ! valid_hash "$archive" "$checksum"; then
        curl --fail --location --silent --show-error --retry 2 -o "$archive.tmp" "$url"
        valid_hash "$archive.tmp" "$checksum"
        mv "$archive.tmp" "$archive"
    fi
    stamp="source/$package/.lumen-source"
    patch="$port/$package.patch"
    fingerprint=$checksum
    if [ -f "$patch" ]; then
        fingerprint=$(sha256sum "$patch")
        fingerprint="$checksum ${fingerprint%% *}"
    fi
    if test -f "$stamp" && test "$(cat "$stamp")" = "$fingerprint"; then
        continue
    fi
    rm -rf "source/$package" "objects/$package"
    rm -f "$prefix/.built-$package"
    tar xf "$archive" -C source
    if [ -f "$patch" ]; then
        (cd "source/$package" && patch -p1 < "$patch")
    fi
    printf '%s\n' "$fingerprint" > "$stamp"
done < "$port/sources.lock"

auto() {
    package=$1
    shift
    binary=${PORT_RELINK:-}
    stamp="$prefix/.built-$package"
    if [ -f "$stamp" ] && [ -z "$binary" ]; then return; fi
    mkdir -p "objects/$package"
    (
        cd "objects/$package"
        if [ ! -f "$stamp" ]; then
            "$work/source/$package/configure" --build=x86_64-pc-linux-gnu \
                --host=x86_64-unknown-linux-musl --prefix="$prefix" \
                --disable-shared --enable-static "$@"
        fi
        if [ -n "$binary" ]; then rm -f "$binary"; fi
        make -j"$jobs"
        make install DESTDIR="${PORT_DESTDIR:-}"
    )
    touch "$prefix/.built-$package"
}

# Static dependencies are isolated from host pkg-config metadata and headers.
auto util-macros-1.20.2
auto xorgproto-2025.1
auto xtrans-1.6.0
auto libXau-1.0.12
auto libXdmcp-1.1.5
auto xcb-proto-1.17.0 PYTHON=/usr/bin/python3
auto libxcb-1.17.0 --disable-devel-docs --disable-dri2 --disable-dri3 --disable-present \
    --disable-glx --disable-xinput --disable-xv --disable-xvmc --disable-record --disable-selinux \
    --disable-damage --disable-dpms --disable-screensaver --disable-res --disable-xf86dri \
    --disable-xfixes --disable-ge --disable-randr --disable-composite --disable-shm \
    ac_cv_search_sendmsg=no ac_cv_func_sendmsg=no
PORT_DESTDIR="$work/libX11-stage" auto libX11-1.8.12 --without-xmlto --disable-xlocaledir \
    --datadir=/usr/share
cp -a "$work/libX11-stage$prefix/." "$prefix/"
auto libXext-1.3.6 --without-xmlto
auto libXrender-0.9.12
auto libxkbfile-1.1.3
if [ ! -f "$prefix/.built-zlib-1.3.1" ]; then
    (cd source/zlib-1.3.1 && ./configure --static --prefix="$prefix" && make -j"$jobs" && make install)
    touch "$prefix/.built-zlib-1.3.1"
fi
auto libfontenc-1.1.8
auto libXfont2-2.0.7 --disable-freetype --disable-fc --without-bzip2
auto libmd-1.1.0

cat > lumen-cross.ini <<CROSS
[binaries]
c = '$CC'
ar = 'ar'
strip = 'strip'
pkg-config = 'pkg-config'

[host_machine]
system = 'lumen'
cpu_family = 'x86_64'
cpu = 'x86_64'
endian = 'little'

[properties]
needs_exe_wrapper = true

[built-in options]
c_args = ['-D__lumen__']
CROSS
if [ ! -f "$prefix/.built-pixman-0.46.4" ]; then
    meson setup objects/pixman-0.46.4 source/pixman-0.46.4 --cross-file "$work/lumen-cross.ini" \
        --prefix "$prefix" --libdir lib --default-library static --buildtype release \
        -Ddemos=disabled -Dtests=disabled
    meson compile -C objects/pixman-0.46.4 -j "$jobs"
    meson install -C objects/pixman-0.46.4 --no-rebuild
    touch "$prefix/.built-pixman-0.46.4"
fi

rustc --edition 2024 --target x86_64-unknown-none --crate-type staticlib \
    -C panic=abort -C opt-level=2 -C relocation-model=static "$port/platform.rs" \
    -o "$prefix/lib/liblumen_x11.a"

server=xorg-server-21.1.26
if [ ! -f "objects/$server/build.ninja" ]; then
    meson setup "objects/$server" "source/$server" --cross-file "$work/lumen-cross.ini" \
        --prefix /usr --libdir lib --buildtype release -Dc_link_args="-L$prefix/lib" \
        -Dxorg=false -Dxnest=false -Dxephyr=false -Dxvfb=true -Dglx=false -Dxdmcp=false \
        -Dsecure-rpc=false -Dipv6=false -Dinput_thread=false -Dudev=false -Dudev_kms=false \
        -Dsystemd_logind=false -Dpciaccess=false -Ddrm=false -Ddri1=false -Ddri2=false -Ddri3=false \
        -Dmitshm=false -Dxselinux=false -Dsha1=libmd -Ddocs=false -Ddevel-docs=false -Ddocs-pdf=false \
        -Ddefault_font_path=/usr/share/X11/fonts/misc,built-ins -Dxkb_dir=/usr/share/X11/xkb \
        -Dlisten_local=false -Dlumen_platform="$prefix/lib/liblumen_x11.a"
fi
meson compile -C "objects/$server" -j "$jobs" Xvfb

manager=jwm-2.4.6
if [ ! -f "source/$manager/Makefile" ]; then
    (
        cd "source/$manager"
        LDFLAGS="$LDFLAGS" LIBS="-lxcb -lXau -lXdmcp -llumen_x11" ./configure \
            --build=x86_64-pc-linux-gnu --host=x86_64-unknown-linux-musl --prefix=/usr \
            --x-includes="$prefix/include" --x-libraries="$prefix/lib" \
            --disable-nls --disable-xft --disable-pango --disable-cairo --disable-rsvg \
            --disable-jpeg --disable-png --disable-xpm --disable-xmu --disable-xinerama
    )
fi
rm -f "source/$manager/src/jwm"
make -C "source/$manager" -j"$jobs"
auto libICE-1.1.2
auto libSM-1.2.6 --without-libuuid
auto libXt-1.3.1 --with-appdefaultdir=/usr/share/X11/app-defaults \
    '--with-xfile-search-path=/usr/share/X11/%T/%N%S'
auto libXmu-1.2.1
auto libXpm-3.5.17 --disable-open-zfile
auto libXaw-1.0.16 --disable-docs
PORT_RELINK=xedit PORT_DESTDIR="$work/xedit-stage" auto xedit-1.2.5 --with-appdefaultdir="$prefix/share/X11/app-defaults" \
    --with-lispdir=/usr/share/X11/xedit/lisp \
    "PKGDEPS_LIBS=$(pkg-config --static --libs xaw7 xmu xt x11)"
PORT_RELINK=xclock auto xclock-1.1.1 --without-xft --without-xkb --with-appdefaultdir="$prefix/share/X11/app-defaults" \
    "XCLOCK_LIBS=$(pkg-config --static --libs xaw7 xmu x11 xproto)"

root="$work/root"
share="$root/usr/share/X11"
fonts="$share/fonts/misc"
mkdir -p "$root/bin" "$fonts" "$share/xkb/rules" "$share/app-defaults" \
    "$share/bitmaps" "$share/locale" "$share/licenses"
install -m 755 "objects/$server/hw/vfb/Xvfb" "source/$manager/src/jwm" "$prefix/bin/xclock" "$root/bin/"
install -m 755 "$work/xedit-stage$prefix/bin/xedit" "$root/bin/"
install -m 644 "$work/xedit-stage$prefix/share/X11/app-defaults/Xedit" \
    "$work/xedit-stage$prefix/share/X11/app-defaults/Xedit-color" \
    "$prefix/share/X11/app-defaults/XClock" "$share/app-defaults/"
cp -R "$work/xedit-stage/usr/share/X11/xedit" "$share/"
install -m 644 source/xbitmaps-1.1.3/menu12 source/xbitmaps-1.1.3/xlogo11 "$share/bitmaps/"
locale="$work/libX11-stage/usr/share/X11/locale"
cp -R "$locale/C" "$locale/en_US.UTF-8" "$locale/iso8859-1" "$share/locale/"
install -m 644 "$locale/compose.dir" "$locale/locale.alias" "$locale/locale.dir" "$share/locale/"
strip "$root/bin/Xvfb" "$root/bin/jwm" "$root/bin/xclock" "$root/bin/xedit"
install -m 644 "source/$manager/example.jwmrc" "$share/jwmrc"
for name in 6x13 7x13 9x15 cursor; do
    case "$name" in
        cursor) package=font-cursor-misc-1.0.4 ;;
        *) package=font-misc-misc-1.1.3 ;;
    esac
    bdftopcf -o "$fonts/$name.pcf" "source/$package/$name.bdf"
done
mkdir -p font-work
(
    cd font-work
    ucs2any "$work/source/font-misc-misc-1.1.3/6x13.bdf" \
        "$work/source/font-util-1.4.1/map-ISO8859-1" ISO8859-1
    bdftopcf -o "$fonts/6x13-ISO8859-1.pcf" 6x13-ISO8859-1.bdf
)
printf '%s\n' 'fixed -misc-fixed-medium-r-semicondensed--13-120-75-75-c-60-iso10646-1' > "$fonts/fonts.alias"
mkfontdir "$fonts"
xkb=xkeyboard-config-2.46
if [ ! -f "objects/$xkb/build.ninja" ]; then
    meson setup "objects/$xkb" "source/$xkb" -Dnls=false -Dcompat-rules=false
fi
ninja -C "objects/$xkb" rules/evdev
xkbcomp -w 0 -R"$work/source/$xkb" -xkm "$port/us.xkb" "$share/xkb/lumen-us.xkm"
cp "objects/$xkb/rules/evdev" "$share/xkb/rules/"
while read -r package _; do
    for name in COPYING LICENSE README; do
        if [ -f "source/$package/$name" ]; then
            cp "source/$package/$name" "$share/licenses/$package"
            break
        fi
    done
done < "$port/sources.lock"
