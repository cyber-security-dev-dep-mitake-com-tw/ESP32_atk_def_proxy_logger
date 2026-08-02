# Julia analysis image. Precompiles dependencies at build time, then runs against
# the shared /data volume written by the backend. By default it watches the events
# file and re-prints a report whenever it grows; override the command to run once.
FROM julia:1.10-bookworm
WORKDIR /app

# Instantiate deps first so they are cached and precompiled into the image layer.
COPY Project.toml ./
RUN julia --project=. -e 'using Pkg; Pkg.instantiate(); Pkg.precompile()'

COPY src ./src
COPY test ./test
COPY run_report.jl watch_report.jl ./

# Shared capture data mounted from the backend.
VOLUME ["/data"]
ENV EVENTS_FILE=/data/events.jsonl

# Watch mode: re-report as new events arrive. Run once instead with:
#   docker compose run --rm analysis julia --project=. run_report.jl /data/events.jsonl
CMD ["julia", "--project=.", "watch_report.jl"]
