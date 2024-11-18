FROM node:22-slim AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y git \
    && rm -rf /var/lib/apt/lists/*

RUN git clone https://github.com/spacebarchat/server.git .

RUN npm ci --include prod --include dev --include optional --include peer
RUN npm run setup
RUN npm prune --production

CMD ["npm", "run", "start"]