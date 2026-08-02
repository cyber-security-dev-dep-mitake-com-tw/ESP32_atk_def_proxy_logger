# Robot Framework runner. Tests an already-running backend container over the
# compose network (START_AGENT:False), so it does not need the Go binary itself.
FROM python:3.12-slim
WORKDIR /tests
COPY requirements.txt ./
RUN pip install --no-cache-dir -r requirements.txt
COPY resources ./resources
COPY suites ./suites

# BASE_URL points at the backend service; START_AGENT False makes the suite wait
# for that backend instead of spawning its own.
ENV BASE_URL=http://backend:8080
CMD ["robot", "--outputdir", "/tests/output", \
     "-v", "START_AGENT:False", "-v", "BASE_URL:http://backend:8080", \
     "suites"]
