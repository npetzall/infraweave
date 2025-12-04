FROM chef AS opa
ARG TARGETARCH

ENV OPA_VERSION=0.69.0

RUN wget https://github.com/open-policy-agent/opa/releases/download/v${OPA_VERSION}/opa_linux_${TARGETARCH}_static \
    -O /usr/local/bin/opa && \
    chmod +x /usr/local/bin/opa