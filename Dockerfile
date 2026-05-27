FROM alpine:3.23
RUN apk add --no-cache ca-certificates opus
COPY seedrelay /app/seedrelay
WORKDIR /app
RUN chmod +x /app/seedrelay
EXPOSE 8000
CMD ["./seedrelay", "--host", "0.0.0.0"]
