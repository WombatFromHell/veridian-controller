FROM rust:slim

RUN cargo install cargo-appimage
RUN apt-get update && apt-get install -y --no-install-recommends file wget
# Download and install appimagetool
RUN wget https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-$(uname -m).AppImage -O /usr/local/bin/appimagetool && \
    chmod +x /usr/local/bin/appimagetool && \
    sed -i 's|AI\x02|\x00\x00\x00|' /usr/local/bin/appimagetool
# Build project and package
WORKDIR /app
COPY . /app/

ARG CI
ENV CI=$CI

ENTRYPOINT [ "/app/entrypoint.sh" ]