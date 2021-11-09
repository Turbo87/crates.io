FROM node:17.0.1-alpine

WORKDIR /app
COPY package.json yarn.lock /app/

RUN yarn install

COPY . /app

ENTRYPOINT ["yarn", "start:staging"]
