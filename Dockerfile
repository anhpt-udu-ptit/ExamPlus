# ==============================================================================
# ExamPlus Multi-Stage Build Dockerfile
# Tạo container tối ưu (< 50MB) chạy độc lập máy chủ phòng thi
# ==============================================================================

# STAGE 1: Builder
FROM rust:1.80-alpine AS builder

RUN apk add --no-cache musl-dev gcc

WORKDIR /app

# Copy toàn bộ mã nguồn
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build release nhị phân độc lập
RUN cargo build --release --bin examplus-app

# ==============================================================================
# STAGE 2: Minimal Runtime
FROM alpine:3.20

# Cài đặt chứng chỉ CA cho kết nối HTTPS ra ngoài
RUN apk add --no-cache ca-certificates tzdata \
    && addgroup -S appgroup && adduser -S appuser -G appgroup

WORKDIR /app

# Copy file nhị phân từ stage builder
COPY --from=builder /app/target/release/examplus-app /app/examplus-app

# Cấp quyền cho user phi đặc quyền
RUN chown -R appuser:appgroup /app
USER appuser

# Cổng dịch vụ phòng thi
EXPOSE 8080

# Chạy ứng dụng
CMD ["/app/examplus-app"]
