# Dashboard Interaction Spec

## 主状态
- `tab`: `all | mine`
- `view_mode`: `list | card`
- `sort_by`: `recent | title`
- `sort_menu_open`: 排序菜单开关
- `create_modal_open`: 创建弹层开关

## 用户动作
- 切换 tab: 只影响 notebook 数据集合，不影响其他状态。
- 切换视图: 只切展示方式，不改数据排序和过滤。
- 打开排序菜单: 选择后立即关闭菜单并刷新排序结果。
- 点击 notebook 行或卡片: 进入该 notebook 的 workspace。
- 更多菜单: 收藏、重命名、删除。
- 新建 notebook: 成功后将新 notebook 插入列表顶部，并跳转进入 workspace。

## 产品约束
- 页面不做“智能推荐图标”。
- 页面不做复杂多维过滤；本轮只保留用户截图中明确出现的动作。
- 如果数据为空，空态只引导创建 notebook，不增加次要 CTA。
