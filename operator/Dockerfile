FROM gcr.io/distroless/cc-debian12

ENV K8S_OPENAPI_ENABLED_VERSION=v1.31

COPY binaries/operator-${TARGETOS}-${TARGETARCH} /usr/local/bin/operator

CMD ["operator"]
