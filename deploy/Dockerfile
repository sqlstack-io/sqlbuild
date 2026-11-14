# Install a released SQLBuild (SQLBUILD_SOURCE=release) or this checkout's wheels; see deploy/README.md.
ARG PYTHON_VERSION=3.12
ARG SQLBUILD_SOURCE=wheels
FROM docker.io/library/python:${PYTHON_VERSION}-slim-bookworm AS install-release
ARG SQLBUILD_VERSION
RUN test -n "$SQLBUILD_VERSION" && python -m pip install --no-cache-dir "sqlbuild==${SQLBUILD_VERSION}"

FROM docker.io/library/python:${PYTHON_VERSION}-slim-bookworm AS install-wheels
COPY dist/*.whl /wheels/
RUN python -m pip install --no-cache-dir /wheels/*.whl && rm -rf /wheels

FROM install-${SQLBUILD_SOURCE} AS runtime
RUN useradd --uid 10001 --create-home sqlbuild \
    && mkdir -p /project/target /project/logs /warehouse \
    && chown sqlbuild:sqlbuild /project /project/target /project/logs /warehouse
ENV PYTHONUNBUFFERED=1
WORKDIR /project
USER 10001:10001
ENTRYPOINT ["sqb"]

FROM runtime AS location
ARG SQLBUILD_GIT_REVISION
RUN test -n "$SQLBUILD_GIT_REVISION"
ENV SQLBUILD_GIT_REVISION=${SQLBUILD_GIT_REVISION}
COPY --chown=10001:10001 deploy/project/ /project/

# The control plane has no Python interpreter, project, adapter, or warehouse credentials.
FROM docker.io/library/debian:bookworm-slim AS server
RUN apt-get update && apt-get install --no-install-recommends -y ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --create-home sqlbuild
COPY --from=runtime /usr/local/bin/sqb /usr/local/bin/sqb
USER 10001:10001
ENTRYPOINT ["sqb"]
CMD ["server", "--config", "/etc/sqlbuild/server.toml"]
