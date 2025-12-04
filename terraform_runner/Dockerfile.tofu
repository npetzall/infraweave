FROM chef AS tofu
ARG TARGETARCH

ENV TOFU_VERSION=1.9.1

RUN wget https://github.com/opentofu/opentofu/releases/download/v${TOFU_VERSION}/tofu_${TOFU_VERSION}_linux_${TARGETARCH}.zip && \
    unzip tofu_${TOFU_VERSION}_linux_${TARGETARCH}.zip && \
    mv tofu /usr/local/bin/terraform && \
    chmod +x /usr/local/bin/terraform
