# This file was generated using a Jinja2 template.
# Please make your changes in `Dockerfile.j2` and then `make` the individual Dockerfiles.

# Using multistage build:
# 	https://docs.docker.com/develop/develop-images/multistage-build/
# 	https://whitfin.io/speeding-up-rust-docker-builds/
####################### VAULT BUILD IMAGE  #######################
# The web-vault digest specifies a particular web-vault build on Docker Hub.
# Using the digest instead of the tag name provides better security,
# as the digest of an image is immutable, whereas a tag name can later
# be changed to point to a malicious image.
#
# To verify the current digest for a given tag name:
# - From https://hub.docker.com/r/vaultwarden/web-vault/tags,
#   click the tag name to view the digest of the image it currently points to.
# - From the command line:
#     $ docker pull vaultwarden/web-vault:v2.21.1
#     $ docker image inspect --format "{{.RepoDigests}}" vaultwarden/web-vault:v2.21.1
#     [vaultwarden/web-vault@sha256:29a4fa7bf3790fff9d908b02ac5a154913491f4bf30c95b87b06d8cf1c5516b5]
#
# - Conversely, to get the tag name from the digest:
#     $ docker image inspect --format "{{.RepoTags}}" vaultwarden/web-vault@sha256:29a4fa7bf3790fff9d908b02ac5a154913491f4bf30c95b87b06d8cf1c5516b5
#     [vaultwarden/web-vault:v2.21.1]
#
FROM vaultwarden/web-vault@sha256:29a4fa7bf3790fff9d908b02ac5a154913491f4bf30c95b87b06d8cf1c5516b5 as vault

########################## BUILD IMAGE  ##########################
FROM rust:1.53 as build

# Debian-based builds support multidb
ARG DB=sqlite,mysql,postgresql

# Build time options to avoid dpkg warnings and help with reproducible builds.
ENV DEBIAN_FRONTEND=noninteractive LANG=C.UTF-8 TZ=UTC TERM=xterm-256color

# Don't download rust docs
RUN rustup set profile minimal

# Install required build libs for armhf architecture.
# To compile both mysql and postgresql we need some extra packages for both host arch and target arch
RUN sed 's/^deb/deb-src/' /etc/apt/sources.list > \
        /etc/apt/sources.list.d/deb-src.list \
    && dpkg --add-architecture armhf \
    && apt-get update \
    && apt-get install -y \
        --no-install-recommends \
        libssl-dev:armhf \
        libc6-dev:armhf \
        libpq5:armhf \
        libpq-dev \
        libmariadb-dev:armhf \
        libmariadb-dev-compat:armhf \
        gcc-arm-linux-gnueabihf \
    && mkdir -p ~/.cargo \
    && echo '[target.armv7-unknown-linux-gnueabihf]' >> ~/.cargo/config \
    && echo 'linker = "arm-linux-gnueabihf-gcc"' >> ~/.cargo/config \
    && echo 'rustflags = ["-L/usr/lib/arm-linux-gnueabihf"]' >> ~/.cargo/config

ENV CARGO_HOME "/root/.cargo"
ENV USER "root"

# Creates a dummy project used to grab dependencies
RUN USER=root cargo new --bin /app
WORKDIR /app

# Copies over *only* your manifests and build files
COPY ./Cargo.* ./
COPY ./rust-toolchain ./rust-toolchain
COPY ./build.rs ./build.rs

# NOTE: This should be the last apt-get/dpkg for this stage, since after this it will fail because of broken dependencies.
# For Diesel-RS migrations_macros to compile with MySQL/MariaDB we need to do some magic.
# We at least need libmariadb3:amd64 installed for the x86_64 version of libmariadb.so (client)
# We also need the libmariadb-dev-compat:amd64 but it can not be installed together with the :armhf version.
# What we can do is a force install, because nothing important is overlapping each other.
RUN apt-get install -y --no-install-recommends libmariadb3:amd64 \
    && apt-get download libmariadb-dev-compat:amd64 \
    && dpkg --force-all -i ./libmariadb-dev-compat*.deb \
    && rm -rvf ./libmariadb-dev-compat*.deb \
    # For Diesel-RS migrations_macros to compile with PostgreSQL we need to do some magic.
    # The libpq5:armhf package seems to not provide a symlink to libpq.so.5 with the name libpq.so.
    # This is only provided by the libpq-dev package which can't be installed for both arch at the same time.
    # Without this specific file the ld command will fail and compilation fails with it.
    && ln -sfnr /usr/lib/arm-linux-gnueabihf/libpq.so.5 /usr/lib/arm-linux-gnueabihf/libpq.so

ENV CC_armv7_unknown_linux_gnueabihf="/usr/bin/arm-linux-gnueabihf-gcc"
ENV CROSS_COMPILE="1"
ENV OPENSSL_INCLUDE_DIR="/usr/include/arm-linux-gnueabihf"
ENV OPENSSL_LIB_DIR="/usr/lib/arm-linux-gnueabihf"
RUN rustup target add armv7-unknown-linux-gnueabihf

# Builds your dependencies and removes the
# dummy project, except the target folder
# This folder contains the compiled dependencies
RUN cargo build --features ${DB} --release --target=armv7-unknown-linux-gnueabihf \
    && find . -not -path "./target*" -delete

# Copies the complete project
# To avoid copying unneeded files, use .dockerignore
COPY . .

# Make sure that we actually build the project
RUN touch src/main.rs

# Builds again, this time it'll just be
# your actual source files being built
RUN cargo build --features ${DB} --release --target=armv7-unknown-linux-gnueabihf

######################## RUNTIME IMAGE  ########################
# Create a new stage with a minimal image
# because we already have a binary built
FROM balenalib/armv7hf-debian:buster

ENV ROCKET_ENV "staging"
ENV ROCKET_PORT=80
ENV ROCKET_WORKERS=10

# hadolint ignore=DL3059
RUN [ "cross-build-start" ]

# Create data folder and Install needed libraries
RUN mkdir /data \
    && apt-get update && apt-get install -y \
    --no-install-recommends \
    openssl \
    ca-certificates \
    curl \
    dumb-init \
    libmariadb-dev-compat \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

# hadolint ignore=DL3059
RUN [ "cross-build-end" ]

VOLUME /data
EXPOSE 80
EXPOSE 3012

# Copies the files from the context (Rocket.toml file and web-vault)
# and the binary from the "build" stage to the current stage
WORKDIR /
COPY Rocket.toml .
COPY --from=vault /web-vault ./web-vault
COPY --from=build /app/target/armv7-unknown-linux-gnueabihf/release/vaultwarden .

COPY docker/healthcheck.sh /healthcheck.sh
COPY docker/start.sh /start.sh

HEALTHCHECK --interval=60s --timeout=10s CMD ["/healthcheck.sh"]

# Configures the startup!
ENTRYPOINT ["/usr/bin/dumb-init", "--"]
CMD ["/start.sh"]
