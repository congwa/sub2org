# 构建 console 静态产物并用 nginx 托管（同源反代 API）。build context = 仓库根（sub2org/）。
FROM node:18-alpine AS build
WORKDIR /web
COPY console/package*.json ./
RUN npm install
COPY console/ ./
RUN npm run build

FROM nginx:1.27-alpine
COPY --from=build /web/dist /usr/share/nginx/html
# nginx.conf 由 compose 挂载到 /etc/nginx/conf.d/default.conf
