FROM python:3.10-slim

ENV PYTHONDONTWRITEBYTECODE=1
ENV PYTHONUNBUFFERED=1
ENV PIP_NO_CACHE_DIR=1

WORKDIR /app

COPY requirements.txt /app/requirements.txt
RUN python -m pip install --upgrade pip && \
    python -m pip install --default-timeout 120 --retries 5 -r /app/requirements.txt

COPY server.py /app/server.py
COPY prepare.py /app/prepare.py

EXPOSE 8765

CMD ["python", "server.py"]
