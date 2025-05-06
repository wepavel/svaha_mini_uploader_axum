FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y curl && \
    curl --proto '=https' --tlsv1.2 -sSfL https://sh.vector.dev | bash -s -- -y && \
    ln -s /root/.vector/bin/vector /usr/local/bin/vector && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/

WORKDIR /app/

COPY ./app /app


