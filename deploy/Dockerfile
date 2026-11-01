# Build wheels from this checkout before building these images; see deploy/README.md.
ARG PYTHON_VERSION=3.12
FROM python:${PYTHON_VERSION}-slim-bookworm AS runtime
COPY dist/*.whl /wheels/
RUN python -m pip install --no-cache-dir /wheels/*.whl && rm -rf /wheels \
    && useradd --uid 10001 --create-home sqlbuild \
    && mkdir /project /warehouse && chown sqlbuild:sqlbuild /project /warehouse
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
FROM debian:bookworm-slim AS server
RUN apt-get update && apt-get install --no-install-recommends -y ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --create-home sqlbuild
COPY --from=runtime /usr/local/bin/sqb /usr/local/bin/sqb
USER 10001:10001
ENTRYPOINT ["sqb"]
CMD ["server", "--host", "0.0.0.0", "--port", "8000", "--launchers", "/etc/sqlbuild/launchers.json"]
