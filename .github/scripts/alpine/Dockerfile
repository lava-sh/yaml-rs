FROM docker.io/library/python:3.14-alpine

RUN apk add --no-cache rust cargo

COPY --from=ghcr.io/astral-sh/uv:latest /uv /bin/
COPY entrypoint.sh /entrypoint.sh

RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]