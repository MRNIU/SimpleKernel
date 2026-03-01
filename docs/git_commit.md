# Git Commit 规范

Based on https://zhuanlan.zhihu.com/p/182553920

- 格式

```
<type>(<scope>): <subject>
```

- type(必须)

  用于说明 git commit 的类别，只允许使用下面的标识

  - feat: 新功能(feature)

  - fix: 修复 bug

  - bug: 发现 bug，但没有修复

  - docs: 文档(documentation)

  - style: 格式(不影响代码运行的变动)

  - refactor: 重构(即不是新增功能，也不是修改 bug 的代码变动)

  - perf: 优化相关，比如提升性能、体验

  - test: 增加测试

  - build: 构建过程或辅助工具的变动

  - revert: 回滚到上一个版本

  - merge: 代码合并

  - sync: 同步主线或分支

  - comment: 修改注释

- scope(可选)
  scope 用于说明 commit 影响的范围，视项目不同而不同。

  例如在 SimpleKernel，可以是 PMM，VMM，include 等。多个 scope 使用英文逗号隔开。如果

  你的修改影响了很多 scope，你可以使用 * 代替，但为了保证原子性，不建议一次修改多个 scope。

- subject(必须)

  subject 是 commit 目的的简短描述，不超过50个字符。

  结尾不加句号或其他标点符号。


根据以上规范 git commit message 将是如下的格式:

```
fix(DAO): 用户查询缺少username属性
feat(Controller): 用户查询接口开发
```
