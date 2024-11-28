FROM bitnami/minideb as builder

WORKDIR /tmp

ENV CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN --mount=type=bind,src=docker/ffmpeg.sh,dst=/mount/ffmpeg.sh \
    /mount/ffmpeg.sh

RUN --mount=type=bind,src=docker/rust.sh,dst=/mount/rust.sh \
    /mount/rust.sh

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        libclang-dev \
        protobuf-compiler \
        patchelf

COPY . .

ARG PROFILE=release

RUN cargo build --profile ${PROFILE} --bin scuffle-image-processor -p scuffle-image-processor --locked

RUN mkdir /out && \
    mv target/${PROFILE}/scuffle-image-processor /out/image-processor && \
    ldd /out/image-processor | grep -o '/[^ ]*' | xargs -I '{}' cp {} /out && \
    patchelf --set-rpath '$ORIGIN'  /out/image-processor

FROM gcr.io/distroless/base-nossl-debian12

LABEL org.opencontainers.image.source=https://github.com/scufflecloud/scuffle
LABEL org.opencontainers.image.description="Scuffle Image Processor" 
LABEL org.opencontainers.image.licenses=AGPL-3.0

WORKDIR /app

ENV LD_LIBRARY_PATH=/app:$LD_LIBRARY_PATH

COPY --from=builder /out /app

STOPSIGNAL SIGTERM

USER 1000

ENTRYPOINT ["/app/image-processor"]
