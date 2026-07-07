FROM rust:latest
WORKDIR /app
COPY . .
RUN chmod +x user_f.sh user_s.sh
RUN apt-get update && apt-get install -y git build-essential \
    && git clone https://github.com/Tiiffi/mcrcon.git \
    && cd mcrcon && make && make install
RUN cargo build --release
CMD ["./target/release/mc_dis_bot"]