
Use a tmpfs (RAM disk) for MySQL in Docker, must move MySQL's storage engine off your hard drive and into system memory.

Stop & remove previous if needed:
```
docker stop kanpad-rs-testdb
docker rm kanpad-rs-testdb
```

Start tmpfs MySQL for testing:
```
docker run -d \
  --name kanpad-rs-testdb \
  -p 3337:3306 \
  -e MYSQL_ROOT_PASSWORD=dev_root_pw \
  -e MYSQL_DATABASE=kanpad \
  --tmpfs /var/lib/mysql:rw,noexec,nosuid,size=1G \
  mysql:8.4 \
  --max-connections=1000
```

check status: `docker logs -f kanpad-rs-testdb`

Run the tests:
```
RUST_BACKTRACE=full DATABASE_URL='mysql://root:dev_root_pw@127.0.0.1:3337/kanpad?charset=utf8mb4' cargo test --test api -- --no-capture
```

