# Build the React bundle, then serve it with nginx. nginx also reverse-proxies
# /api and /ws to the backend container so the browser talks to a single origin
# (mirroring the Vite dev proxy).
FROM node:20-alpine AS build
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM nginx:1.27-alpine
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/dist /usr/share/nginx/html
EXPOSE 80
