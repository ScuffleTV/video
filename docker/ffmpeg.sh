set -ex

apt-get update
apt-get install -y --no-install-recommends \
        tar \
        make \
        zip \
        unzip \
        curl \
        wget \
        git \
        ssh \
        ca-certificates \
        pkg-config \
        gnupg2 \
        cmake \
        clang-format \
        ninja-build \
        nasm \
        yasm \
        meson \
        libtool \
        autoconf \
        automake \
        build-essential \
        libpng-dev \
        libjpeg-dev \
        libtiff-dev \
        libpng16-16 \
        libjpeg62 \
        libtiff6


git clone https://github.com/scufflecloud/external.git --depth 1 --recurse-submodule --shallow-submodules /tmp/external
/tmp/external/build.sh --prefix /usr/local --build "x264 x265 svt-av1 libvpx dav1d ffmpeg"
ldconfig
