
# Docker 开发环境配置

本文档介绍如何使用 Docker 搭建 SimpleKernel 的开发环境，支持两种方式：使用预构建镜像或自建镜像。

## 目录

- [快速开始（推荐）](#快速开始推荐)
- [自建镜像](#自建镜像)
- [SSH 配置](#ssh-配置)
- [VSCode 远程开发](#vscode-远程开发)
- [常用命令](#常用命令)

## 快速开始（推荐）

### 1. 拉取并运行预构建镜像

```shell
# 进入项目目录
cd SimpleKernel

# 拉取最新镜像
docker pull ptrnull233/simple_kernel:latest

# 启动容器（包含完整的挂载配置）
docker run --name SimpleKernel-container -itd \
  -p 233:22 \
  -v ./:/root/SimpleKernel \
  -v ~/.ssh:/root/.ssh \
  -v ~/.gitconfig:/root/.gitconfig \
  ptrnull233/simple_kernel:latest

# 进入容器
docker exec -it SimpleKernel-container /bin/zsh
```

### 2. 验证环境

进入容器后，验证开发工具是否正常：

```shell
# 检查编译器
gcc --version
clang --version

# 检查交叉编译工具链
aarch64-linux-gnu-gcc --version
riscv64-linux-gnu-gcc --version

# 检查构建工具
cmake --version
make --version

# 尝试构建项目
cd /root/SimpleKernel
mkdir -p build && cd build
cmake ..
make -j$(nproc)
```

## 自建镜像

如果需要自定义镜像或预构建镜像不可用：

### 1. 构建镜像

```shell
cd SimpleKernel
docker buildx build -t simple_kernel-docker-image ./tools
```

### 2. 启动自建镜像容器

```shell
docker run --name SimpleKernel-container -itd \
  -p 233:22 \
  -v ./:/root/SimpleKernel \
  -v ~/.ssh:/root/.ssh \
  -v ~/.gitconfig:/root/.gitconfig \
  simple_kernel-docker-image
```

### 3. 进入容器

```shell
docker exec -it SimpleKernel-container /bin/zsh
```

## SSH 配置

为了更好的开发体验，可以配置 SSH 远程访问：

### 1. 生成 SSH 密钥（本地）

```shell
# 检查是否已存在 SSH 密钥
ls ~/.ssh/

# 如果不存在，生成新的 RSA 密钥
# 将 <your-email> 替换为你的邮箱
ssh-keygen -t rsa -b 4096 -C "<your-email>"

# 查看公钥内容
cat ~/.ssh/id_rsa.pub
```

### 2. 配置容器 SSH 访问

```shell
# 通过 SSH 连接到容器（首次需要密码）
ssh -p 233 zone@localhost
# 默认密码：zone

# 在容器内创建 SSH 目录和授权文件
mkdir -p /home/zone/.ssh
touch /home/zone/.ssh/authorized_keys
chmod 700 /home/zone/.ssh
chmod 600 /home/zone/.ssh/authorized_keys

# 将本地公钥内容添加到 authorized_keys
# 可以通过 docker exec 或直接编辑文件
```

### 3. 验证 SSH 免密登录

```shell
# 现在应该可以免密登录
ssh -p 233 zone@localhost
```

## VSCode 远程开发

### 1. 安装插件

在 VSCode 中安装以下插件：
- `Remote - SSH`
- `Remote - Containers` (可选)

### 2. 配置 SSH 连接

1. 打开命令面板：`Ctrl+Shift+P` (Windows/Linux) 或 `Cmd+Shift+P` (macOS)
2. 输入：`Remote-SSH: Add New SSH Host...`
3. 输入 SSH 命令：`ssh -p 233 zone@localhost`
4. 选择配置文件保存位置

### 3. 连接并打开项目

1. 打开命令面板：`Ctrl+Shift+P` / `Cmd+Shift+P`
2. 输入：`Remote-SSH: Connect to Host...`
3. 选择 `zone@localhost`
4. 在新窗口中打开文件夹：`/home/zone/SimpleKernel`

### 4. 推荐插件（远程环境）

在远程环境中安装以下插件以获得更好的开发体验：
- C/C++ Extension Pack
- CMake Tools
- GitLens
- Clang-Format

## 常用命令

### 容器管理

```shell
# 查看容器状态
docker ps -a

# 启动已停止的容器
docker start SimpleKernel-container

# 停止容器
docker stop SimpleKernel-container

# 重启容器
docker restart SimpleKernel-container

# 删除容器
docker rm SimpleKernel-container

# 查看容器日志
docker logs SimpleKernel-container
```

### 镜像管理

```shell
# 查看本地镜像
docker images

# 删除镜像
docker rmi simple_kernel-docker-image

# 清理未使用的镜像
docker image prune
```

### 文件传输

```shell
# 从容器复制文件到主机
docker cp SimpleKernel-container:/path/to/file /host/path/

# 从主机复制文件到容器
docker cp /host/path/file SimpleKernel-container:/path/to/
```

## 故障排除

### 常见问题

1. **端口 233 已被占用**
   ```shell
   # 检查端口占用
   lsof -i :233
   # 或使用其他端口
   docker run -p 234:22 ...
   ```

2. **容器启动失败**
   ```shell
   # 查看详细错误信息
   docker logs SimpleKernel-container
   ```

3. **挂载目录权限问题**
   ```shell
   # 检查目录权限
   ls -la ./
   # 确保 Docker 有权限访问挂载目录
   ```

4. **SSH 连接失败**
   ```shell
   # 检查容器是否在运行
   docker ps
   # 检查 SSH 服务状态
   docker exec -it SimpleKernel-container systemctl status ssh
   ```

### 重置环境

如果遇到无法解决的问题，可以重置整个环境：

```shell
# 停止并删除容器
docker stop SimpleKernel-container
docker rm SimpleKernel-container

# 重新创建容器
docker run --name SimpleKernel-container -itd \
  -p 233:22 \
  -v ./:/root/SimpleKernel \
  -v ~/.ssh:/root/.ssh \
  -v ~/.gitconfig:/root/.gitconfig \
  ptrnull233/simple_kernel:latest
```
