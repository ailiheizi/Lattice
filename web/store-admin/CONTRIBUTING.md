# 贡献指南

感谢您对 Lattice Store 管理面板的关注！我们欢迎各种形式的贡献。

## 如何贡献

### 报告问题

发现 Bug 或有功能建议？请创建 Issue：

1. 搜索现有 Issue，避免重复
2. 使用清晰的标题
3. 详细描述问题或建议
4. 提供复现步骤（Bug）
5. 附上截图或错误日志

**Bug 报告模板**:
```markdown
**描述**
简要描述问题

**复现步骤**
1. 打开管理面板
2. 点击 XXX
3. 看到错误

**期望行为**
应该显示 XXX

**实际行为**
显示了 XXX

**环境**
- 浏览器: Chrome 120
- Store 版本: 1.0.0
- 操作系统: Windows 11

**截图**
如果适用，添加截图

**错误日志**
```
控制台错误信息
```
```

### 提交代码

#### 准备工作

1. Fork 项目仓库
2. 克隆到本地
```bash
git clone https://github.com/your-username/Lattice.git
cd Lattice/web/store-admin
```

3. 创建功能分支
```bash
git checkout -b feature/your-feature-name
```

#### 开发规范

**代码风格**

JavaScript:
```javascript
// 使用 ES6+ 语法
// 使用 const/let，避免 var
// 使用箭头函数
// 添加必要注释

class MyModule {
    constructor() {
        this.data = [];
    }

    async loadData() {
        try {
            const response = await api.getData();
            this.data = response;
        } catch (error) {
            console.error('Failed to load data:', error);
        }
    }
}
```

CSS:
```css
/* 使用 CSS 变量 */
/* 遵循 BEM 命名规范 */
/* 移动端优先 */

.module-name {
    display: flex;
    gap: var(--spacing);
}

.module-name__element {
    color: var(--primary);
}

.module-name--modifier {
    font-weight: bold;
}
```

HTML:
```html
<!-- 语义化标签 -->
<!-- 可访问性属性 -->
<!-- 清晰的结构 -->

<section class="dashboard">
    <h2>Dashboard</h2>
    <div class="stats-grid" role="region" aria-label="Statistics">
        <!-- content -->
    </div>
</section>
```

**命名约定**

- 文件名: `kebab-case.js`
- 类名: `PascalCase`
- 函数名: `camelCase`
- 常量: `UPPER_SNAKE_CASE`
- CSS 类: `kebab-case`

**注释规范**

```javascript
/**
 * 加载消息列表
 * @param {string} contactId - 联系人 ID（可选）
 * @returns {Promise<Array>} 消息数组
 */
async loadMessages(contactId = null) {
    // 实现代码
}
```

#### 提交规范

使用语义化提交信息：

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Type 类型**:
- `feat`: 新功能
- `fix`: Bug 修复
- `docs`: 文档更新
- `style`: 代码格式（不影响功能）
- `refactor`: 重构
- `perf`: 性能优化
- `test`: 测试相关
- `chore`: 构建/工具相关

**示例**:
```
feat(dashboard): add real-time chart updates

- Implement WebSocket connection
- Update charts every 5 seconds
- Add connection status indicator

Closes #123
```

#### 测试

提交前请测试：

1. **功能测试**
   - 测试新功能是否正常工作
   - 测试是否影响现有功能
   - 测试边界情况

2. **浏览器测试**
   - Chrome
   - Firefox
   - Safari
   - Edge

3. **响应式测试**
   - 桌面端（1920x1080）
   - 平板端（768x1024）
   - 移动端（375x667）

4. **API 测试**
   - 使用 test.html 测试 API
   - 测试错误处理
   - 测试加载状态

#### 提交 Pull Request

1. 推送到您的 Fork
```bash
git push origin feature/your-feature-name
```

2. 创建 Pull Request
   - 清晰的标题
   - 详细的描述
   - 关联相关 Issue
   - 添加截图（UI 变更）

3. 等待审查
   - 回应审查意见
   - 及时更新代码
   - 保持沟通

**PR 模板**:
```markdown
## 变更说明
简要描述此 PR 的目的

## 变更类型
- [ ] Bug 修复
- [ ] 新功能
- [ ] 重构
- [ ] 文档更新
- [ ] 性能优化

## 测试
- [ ] 已在 Chrome 测试
- [ ] 已在 Firefox 测试
- [ ] 已测试响应式布局
- [ ] 已测试 API 集成

## 截图
如果有 UI 变更，请添加截图

## 关联 Issue
Closes #123

## 检查清单
- [ ] 代码遵循项目规范
- [ ] 已添加必要注释
- [ ] 已更新相关文档
- [ ] 已测试所有功能
- [ ] 无控制台错误
```

### 文档贡献

文档同样重要！您可以：

- 修正错别字
- 改进说明
- 添加示例
- 翻译文档

### 设计贡献

欢迎提供：

- UI/UX 改进建议
- 图标设计
- 配色方案
- 交互优化

## 开发环境设置

### 必需工具

- 现代浏览器（Chrome/Firefox）
- 文本编辑器（VS Code 推荐）
- Git
- Python 或 Node.js（用于本地服务器）

### VS Code 推荐插件

- ESLint
- Prettier
- Live Server
- GitLens
- Path Intellisense

### 本地开发

1. 启动 Store 服务
```bash
cd Lattice
cargo run --bin lattice-store
```

2. 启动管理面板
```bash
cd web/store-admin
python -m http.server 8080
```

3. 访问 http://localhost:8080

### 调试技巧

**浏览器开发者工具**
- Console: 查看日志和错误
- Network: 检查 API 请求
- Elements: 调试 CSS
- Application: 查看存储

**常用调试代码**
```javascript
// 打印变量
console.log('data:', data);

// 断点调试
debugger;

// 性能测试
console.time('operation');
// ... code ...
console.timeEnd('operation');

// 追踪函数调用
console.trace();
```

## 代码审查标准

审查者会检查：

1. **功能性**
   - 是否实现了预期功能
   - 是否有 Bug
   - 边界情况处理

2. **代码质量**
   - 代码可读性
   - 命名规范
   - 注释完整性
   - 无重复代码

3. **性能**
   - 无性能瓶颈
   - 合理的算法复杂度
   - 资源使用优化

4. **安全性**
   - XSS 防护
   - 输入验证
   - 敏感信息处理

5. **兼容性**
   - 浏览器兼容
   - 响应式设计
   - 降级方案

6. **文档**
   - 代码注释
   - API 文档
   - 使用说明

## 发布流程

维护者负责发布新版本：

1. 更新版本号
2. 更新 CHANGELOG.md
3. 创建 Git tag
4. 发布 Release
5. 更新文档

## 社区准则

### 行为规范

- 尊重他人
- 建设性反馈
- 包容不同观点
- 专注技术讨论

### 沟通渠道

- GitHub Issues: Bug 报告和功能请求
- GitHub Discussions: 一般讨论
- Pull Requests: 代码审查

## 获得帮助

遇到问题？

1. 查看文档（README, INSTALL, FEATURES）
2. 搜索现有 Issue
3. 使用 test.html 测试 API
4. 创建新 Issue 寻求帮助

## 认可贡献者

所有贡献者将被列入：
- CHANGELOG.md
- 项目 README
- Contributors 页面

## 许可证

贡献的代码将采用与项目相同的许可证（MIT）。

## 感谢

感谢您的贡献，让 Lattice 变得更好！

---

**快速链接**
- [README](README.md)
- [安装指南](INSTALL.md)
- [功能详解](FEATURES.md)
- [API 文档](API.md)
- [更新日志](CHANGELOG.md)
