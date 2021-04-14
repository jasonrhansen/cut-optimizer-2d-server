FROM ekidd/rust-musl-builder:stable as builder

USER root

RUN USER=root cargo new --bin cut-optimizer-2d-server
WORKDIR /home/rust/src/cut-optimizer-2d-server
COPY ./Cargo.* ./
RUN cargo build --release
RUN rm src/*.rs

ADD . ./

RUN rm ./target/x86_64-unknown-linux-musl/release/deps/cut_optimizer_2d_server*
RUN cargo build --release


FROM alpine:latest

ARG APP=/usr/src/app

EXPOSE 3030

ENV TZ=Etc/UTC \
    APP_USER=appuser

RUN addgroup -S $APP_USER \
    && adduser -S -g $APP_USER $APP_USER

RUN apk update \
    && apk add --no-cache ca-certificates tzdata \
    && rm -rf /var/cache/apk/*

COPY --from=builder /home/rust/src/cut-optimizer-2d-server/target/x86_64-unknown-linux-musl/release/cut-optimizer-2d-server ${APP}/cut-optimizer-2d-server

RUN chown -R $APP_USER:$APP_USER ${APP}

USER $APP_USER
WORKDIR ${APP}

CMD ["./cut-optimizer-2d-server", "-vv"]
