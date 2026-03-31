# 动态止亏机制 (Dynamic Stop-Loss)

## 概述

动态止亏是一种主动订单管理策略，通过持续监控市场条件和预测变化，自动撤销不再有优势的订单。

## 实现位置

- **代码**: `backend/core/order_manager.py`
- **配置**: `backend/config.py` (ORDER_MANAGEMENT_* 配置项)
- **调度**: `backend/core/scheduler.py` (order_management_job)

## 工作原理

### 1. 触发条件

每 5 分钟自动运行（可配置），检查所有挂单。

### 2. 检查逻辑

对每个挂单执行以下检查：

#### 2.1 Edge 消失检测
```python
# 重新计算当前 edge
current_edge = abs(model_prob - market_prob)

# 如果 edge < 8%，撤单
if current_edge < MIN_EDGE_THRESHOLD:
    cancel_order(order_id)
    reason = "Edge disappeared"
```

**原理**: 市场价格已修正，或 ensemble forecast 更新导致优势消失。

#### 2.2 Forecast 反转检测
```python
# 检查预测方向是否改变
if current_direction != original_direction:
    cancel_order(order_id)
    reason = "Forecast reversed"
```

**原理**: Ensemble forecast 更新后，预测方向完全反转。

#### 2.3 时间衰减检测（可选）
```python
# 离结算太近且 edge 不够大
if days_to_resolution <= 1:
    min_edge = MIN_EDGE_THRESHOLD * 1.5
    if current_edge < min_edge:
        cancel_order(order_id)
        reason = "Edge too small for close resolution"
```

**原理**: 越接近结算时间，要求更高的 edge 才值得持有。

#### 2.4 市场价格大幅变动（可选）
```python
# 市场价格已大幅修正
price_change = abs(current_price - entry_price)
if price_change > 0.30:
    cancel_order(order_id)
    reason = "Market price moved significantly"
```

**原理**: 市场已充分修正，机会已消失。

### 3. 撤单流程

```
1. 获取所有 open orders (get_open_orders)
2. 从数据库查找对应的 Trade 记录
3. 重新生成信号 (generate_weather_signal)
4. 检查是否满足撤单条件
5. 如果需要撤单：
   - 调用 cancel_order(order_id)
   - 更新 Trade.status = "canceled"
   - 记录 cancel_reason
   - 记录 canceled_at 时间戳
```

## 配置参数

```python
# config.py

ORDER_MANAGEMENT_ENABLED: bool = True
ORDER_MANAGEMENT_INTERVAL_SECONDS: int = 300  # 5 分钟
ORDER_MIN_EDGE_THRESHOLD: float = 0.08  # 8%
ORDER_PRICE_CHANGE_THRESHOLD: float = 0.30  # 30%
```

## 数据库变更

Trade 表新增字段：

```sql
ALTER TABLE trades ADD COLUMN order_id VARCHAR;
ALTER TABLE trades ADD COLUMN status VARCHAR DEFAULT 'pending';
ALTER TABLE trades ADD COLUMN canceled_at DATETIME;
ALTER TABLE trades ADD COLUMN cancel_reason VARCHAR;
```

## 日志输出

```
[INFO] Managing open orders...
[DATA] Order management complete: total=2, canceled_edge=1, kept=1
[SUCCESS] Canceled 1 orders (edge: 1, reversed: 0)
```

## 统计指标

```python
{
    "total_orders": 10,
    "canceled_edge_disappeared": 3,
    "canceled_forecast_reversed": 1,
    "kept": 5,
    "errors": 1
}
```

## 优势

1. **主动风险管理**: 不等待结算，主动退出不利仓位
2. **适应性强**: 自动适应 ensemble forecast 更新
3. **避免损失**: 防止 edge 消失后继续持有
4. **节省资金**: 撤单后资金可用于更好的机会

## 限制

1. **仅限挂单**: 只能管理 open orders，已成交订单无法撤回
2. **依赖 forecast**: 需要 ensemble forecast 更新及时
3. **网络延迟**: 撤单需要 API 调用，可能有延迟

## 未来优化

1. **部分撤单**: 支持只撤销部分仓位
2. **分级止亏**: 不同 edge 水平采用不同撤单阈值
3. **机器学习**: 用历史数据训练最优撤单时机
4. **组合级止亏**: 基于整个组合的 P&L 动态调整

## 示例场景

### 场景 1: Edge 消失

```
初始状态:
  订单: Chicago >68°F @ 58.4%
  模型: 0% (edge = 58.4%)
  
30分钟后:
  市场价: 15% (其他交易者也更新了 forecast)
  模型: 0% (edge = 15%)
  
结果:
  edge = 15% > 8% → 保留订单
  
再过 30 分钟:
  市场价: 5%
  模型: 0% (edge = 5%)
  
结果:
  edge = 5% < 8% → 撤单 ✅
```

### 场景 2: Forecast 反转

```
初始状态:
  订单: Chicago 62-63°F @ 3% (UP)
  模型: 3.2% (edge = 0.2%)
  
4小时后 (forecast 更新):
  Ensemble mean: 58.4°F → 64.5°F
  
结果:
  新模型: 15% (在 [62,63] 区间)
  方向: UP → UP (未反转)
  
再过 4小时:
  Ensemble mean: 64.5°F → 61.0°F
  
结果:
  新模型: 1% (在 [62,63] 区间)
  方向: UP → DOWN (反转) → 撤单 ✅
```

## 监控建议

1. **撤单率**: 监控撤单比例，过高可能说明策略过于激进
2. **撤单原因**: 分析 edge 消失 vs forecast 反转的比例
3. **时间分布**: 观察撤单通常发生在下单后多久
4. **盈亏影响**: 对比撤单 vs 持有到结算的盈亏

---

**相关文档**:
- [Weather Trading Strategy](./weather_trading.md)
- [Order Management API](./api.md#order-management)
- [Configuration Guide](./configuration.md)
