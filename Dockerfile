FROM docker.io/library/rust:slim-bullseye

RUN cargo install cargo-appimage
RUN apt-get update && apt-get install -y --no-install-recommends file wget ca-certificates
# Download and install appimagetool
RUN wget https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-$(uname -m).AppImage -O /usr/local/bin/appimagetool && \
  chmod +x /usr/local/bin/appimagetool && \
  sed -i 's|AI\x02|\x00\x00\x00|' /usr/local/bin/appimagetool
# Build project and package
WORKDIR /app
COPY . /app/

ENTRYPOINT [ "/app/entrypoint.sh" ]
