# XaC MVP ゲーム仕様書

**文書種別**: MVP実装仕様書  
**対象読者**: ゲームエンジン・ランタイム・UI・ゲームデザイン担当開発者  
**プロジェクト名**: XaC  
**読み仮称**: ザック / エックス・アズ・コード  
**作成日**: 2026-05-31  
**バージョン**: MVP-0.1

---

## 0. この文書の目的

本書は、これまで議論したゲーム **XaC** のMVP仕様を、開発担当者がそのまま実装に着手できる粒度でまとめたものである。

XaCは、以下を組み合わせたゲームである。

- Mindustry風のグリッド上の工場・物流・防衛・RTS
- The Farmer Was Replaced風の「コードで機械を自動化する」楽しさ
- IaCの発想を拡張した、RTS as Code / Logistics as Code / Factory as Code / Combat as Code
- 左側にグリッド世界、右側にコードエディタを持つIDE型UI
- WebAssemblyによる多言語対応のプレイヤーコード実行環境

ただし、MVPでは「複雑なプログラムを書くゲーム」ではなく、**プリセットの機械を置くだけで遊べ、必要に応じて短いコードや設定を編集すると基地が賢くなるゲーム**を目指す。

---

## 1. 一文コンセプト

**XaCは、プレイヤーがグリッド上に工場・物流・防衛設備を配置し、各ブロックやドローンに紐づくWebAssemblyコードを編集して、資源生産・物流・防衛・RTS行動を自動化する「基地OS構築RTS」である。**

---

## 2. デザイン原則

### 2.1 プレイヤーに長いコードを書かせない

XaCではコードを書けるが、MVPの中心体験は「大量のコードを書く」ことではない。

通常プレイでは以下を重視する。

1. プリセットを置くだけで動く。
2. `Edit` を押すと短い設定・短いコードが開く。
3. 1〜5行の変更でも効果が見える。
4. 上級者はライブラリやWasmモジュールを使って深く最適化できる。

### 2.2 API制限ではなく、物理的なCapability制にする

プレイヤーコードの言語機能をゲーム進行で不自然に制限しない。

代わりに、各ブロック・ユニットが持つ物理的な能力に応じて、呼べるゲームAPIが変わる。

例:

- `turret` は `scan_enemies` / `attack` を持つ。
- `router` は `push` / `push_any` / `output_available` を持つ。
- `carrier_drone` は `move_to` / `load` / `unload` / `battery_ratio` を持つ。
- `drill` は `mine` / `output_blocked` を持つ。

これは「関数が解禁されていない」のではなく、「その機械にはその身体機能がない」ことを意味する。

### 2.3 コードは基地OSの一部

プレイヤーが作るのは単なる防衛ラインではなく、以下を持つ分散システムである。

- ブロックごとのローカルCPU
- 配線で構成される複数ネットワーク
- ネットワークごとのCPUプール
- ネットワーク共有変数
- ドローンのバッテリーと処理速度
- プリセット、フォーク、共有コード、ライブラリ

### 2.4 処理速度はゲーム内資源である

すべてのプログラム可能ブロックには低速なローカルCPUがある。配線でネットワークに接続すると、ネットワーク上のCPU資源が配分され、処理速度が上がる。

MVPでは、処理速度はWebAssemblyランタイムのfuelに対応させる。

### 2.5 電源ケーブルとネットワークケーブルは分けない

MVPでは、電力・ネットワーク・CPU共有のための接続はすべて **wire** で表現する。

プレイヤーが複数種類のケーブルを管理する必要はない。

---

## 3. MVPスコープ

### 3.1 MVPで実装するもの

MVPで必須とする要素は以下。

- 2Dグリッドマップ
- コア拠点
- 配線によるネットワーク形成
- ネットワークごとのCPUプール
- WebAssemblyコード実行
- fuelによるCPU資源消費
- 左グリッド / 右エディタのUI
- プリセットブロック配置
- プリセット編集時の自動コピー
- 自作ブロックの `edit` / `fork + edit`
- 最小ブロックセット
- 最小敵セット
- 輸送ドローン1種
- 共有変数ストア
- 物流asCodeの最小形
- 防衛asCodeの最小形

### 3.2 MVPで明示的に後回しにするもの

以下は設計思想として残すが、MVP実装の必須要件ではない。

- data_bridge: CPUは共有せず変数だけ共有するブロック
- network_switch: ネットワークの論理分割・接続切替
- radar専用ブロック
- repair_drone / scout_drone / combat_drone
- outpost_core
- shield / artillery / advanced_factory
- センサー信頼度システム
- 敵のジャマー、スプーファー、地中敵、敵拠点
- Blueprint + Codeの高度版
- typed network store の完全実装
- PvP
- 完全なイベント駆動システム

ただし、将来導入できるようにアーキテクチャを閉じない。

---

## 4. 基本ゲームループ

MVPのプレイサイクルは以下。

1. コア周辺に `drill` を置いて資源を掘る。
2. `conveyor` と `router` で資源を流す。
3. `assembler` で弾薬や中間素材を作る。
4. `turret` を置いて敵ウェーブを迎撃する。
5. 必要に応じてブロックのプリセットコードを編集する。
6. `cpu_node` を置き、処理速度を改善する。
7. 遠隔地に採掘設備を作るが、配線距離やCPU配分が課題になる。
8. `drone_port` と `carrier_drone` で弾薬・資源輸送を自動化する。
9. 敵の `wire_cutter` によって配線が破壊され、ネットワークが分断される。
10. プレイヤーは配線防衛、CPUノード増設、ローカルCPUだけで動くfallback的コードを考える。

---

## 5. ゲーム世界

### 5.1 グリッド

- マップは2Dタイルグリッド。
- MVP標準サイズは `64 x 64`。
- 1タイルに基本1ブロック。
- 大型ブロックはMVPでは不要。将来 `2x2` や `3x3` を導入可能にする。
- 座標は整数 `x, y`。
- 方角は `north`, `east`, `south`, `west`。

### 5.2 タイル属性

各タイルは以下を持つ。

- 地形種別
- 資源種別またはなし
- 建設可否
- 敵通行可否
- ワイヤ接続可否
- ブロック占有情報

### 5.3 tick

MVPのゲームシミュレーションは固定tickで進む。

推奨:

- シミュレーションtick: 20 tick/sec
- 描画: 任意フレームレート
- Wasm実行予算配布: tickごと

---

## 6. 資源

ユーザー指定により、正式な資源設計はMVP時点では固定しない。

ただしMVP実装には最低限の物流・生産・弾薬が必要なため、仮資源を用いる。

### 6.1 MVP仮資源

| item_id | 用途 |
|---|---|
| `ore` | drillから得られる基礎資源 |
| `plate` | assemblerでoreから作る中間素材 |
| `ammo` | turretが消費する弾薬 |
| `cpu_part` | cpu_nodeの建設コスト用。MVPでは任意 |
| `drone_part` | carrier_drone製造用。MVPでは任意 |

### 6.2 実装方針

資源定義はデータ駆動にする。

推奨ファイル:

```toml
# resources.toml
[[item]]
id = "ore"
display_name = "Ore"
stack_size = 100

[[item]]
id = "plate"
display_name = "Plate"
stack_size = 100

[[item]]
id = "ammo"
display_name = "Ammo"
stack_size = 100
```

将来、Copper / Iron / Carbon / Silica / Water / Crystal 等に拡張できるよう、ゲームロジック側に資源名をハードコードしない。

---

## 7. ブロック一覧: MVP最小セット

MVPに必要な最小ブロックは以下の10種類。

| block_id | 役割 | プログラム可能 | MVP必須 |
|---|---|---:|---:|
| `core` | 初期拠点、初期CPU、初期ストレージ、初期ネットワーク | 一部 | 必須 |
| `wire` | 電力・ネットワーク・CPU共有を兼ねる配線 | いいえ | 必須 |
| `cpu_node` | network_cpu_poolを増やす | いいえ | 必須 |
| `drill` | 資源タイルから資源を掘る | 任意 | 必須 |
| `conveyor` | アイテムを一方向に運ぶ | 原則いいえ | 必須 |
| `router` | アイテムを分岐する。物流asCodeの入口 | はい | 必須 |
| `storage` | アイテムを貯蔵する | 任意 | 必須 |
| `assembler` | 入力から出力を作る | はい | 必須 |
| `turret` | 弾薬を消費して敵を撃つ | はい | 必須 |
| `drone_port` | ドローン充電・同期・配送ジョブ管理 | はい | 必須 |

ユニットとして以下を実装する。

| unit_id | 役割 | プログラム可能 | MVP必須 |
|---|---|---:|---:|
| `carrier_drone` | アイテム配送 | はい | 必須 |

---

## 8. 各ブロック仕様

### 8.1 core

#### 役割

- プレイヤーの初期拠点。
- 初期ストレージを持つ。
- 初期CPUを持つ。
- wireで接続されたネットワークにCPUを提供する。
- network store の初期ノードとして機能する。

#### MVPパラメータ例

```toml
[core]
size = [1, 1]
local_cpu_rate = 0
network_cpu_output = 120
storage_capacity = 1000
```

#### 備考

コア自体もプログラム可能にできるが、MVPでは必須ではない。将来、全体戦略やグローバル物流をコアコードに書かせる。

---

### 8.2 wire

#### 役割

- 配線。
- 電力・ネットワーク・CPU共有をまとめて扱う。
- wireでつながった連結成分が1つの network になる。

#### 仕様

- コードなし。
- 敵に破壊される可能性がある。
- `wire_cutter` の優先攻撃対象。
- wireが破壊されるとネットワークが分断され、CPU配分・storeアクセスが変化する。

---

### 8.3 cpu_node

#### 役割

- 接続中networkのCPUプールを増やす。

#### MVPパラメータ例

```toml
[cpu_node]
network_cpu_output = 80
active_device = false
```

#### 備考

`cpu_node` 自身は通常コードを持たず、CPU配分対象には入らない。

---

### 8.4 drill

#### 役割

- 資源タイルから `ore` を生成する。
- 生成物を隣接conveyorまたはstorageへ出す。

#### プリセット挙動

- 出力先が空いていれば採掘。
- 出力先が詰まっていれば停止。

#### 任意コードAPI

- `mine()`
- `output_blocked() -> bool`
- `output(item_kind) -> bool`
- `ore_kind() -> item_kind`

#### プリセットコード例

```pseudo
export tick(self):
    if self.output_blocked():
        return
    self.mine()
```

---

### 8.5 conveyor

#### 役割

- アイテムを一方向へ運ぶ。

#### MVP仕様

- 原則コードなし。
- 向きだけ持つ。
- 1タイルあたり1アイテムまたは小キューを持つ。

#### 備考

将来、プログラム可能conveyorやfilter conveyorを追加可能。

---

### 8.6 router

#### 役割

- アイテム分岐。
- 物流asCodeの最小体験。

#### イベント

- `on_item(self, item)`

#### API

- `push(dir, item) -> bool`
- `push_any(item) -> bool`
- `output_available(dir) -> bool`
- `item_kind(item) -> string`
- `net_store_get(key) -> value`
- `net_store_set(key, value) -> void`

#### プリセットコード例

```pseudo
export on_item(self, item):
    self.push_any(item)
```

#### 編集例

```pseudo
export on_item(self, item):
    if item.kind == "ammo":
        if self.push("east", item):
            return
    self.push_any(item)
```

---

### 8.7 storage

#### 役割

- アイテム貯蔵。
- 在庫参照。

#### MVP仕様

- 基本コードなし。
- `storage` は同一networkに対して在庫情報を提供する。
- 必要なら `storage` をクリックして在庫を見られる。

#### API

他ブロックから以下のように参照できる。

- `stock_count(item_kind) -> int`
- `stock_capacity(item_kind) -> int`
- `has_space(item_kind, amount) -> bool`

---

### 8.8 assembler

#### 役割

- 入力資源から出力資源を作る。
- 生産レシピをコードで切り替える。

#### MVPレシピ例

```toml
[[recipe]]
id = "plate"
inputs = { ore = 2 }
outputs = { plate = 1 }
time_ticks = 20

[[recipe]]
id = "ammo"
inputs = { plate = 1 }
outputs = { ammo = 2 }
time_ticks = 20
```

#### API

- `set_recipe(recipe_id) -> bool`
- `current_recipe() -> string`
- `can_produce() -> bool`
- `produce() -> bool`
- `input_count(item_kind) -> int`
- `output_count(item_kind) -> int`
- `net_store_get(key) -> value`

#### プリセットコード例

```pseudo
export tick(self):
    if self.output_count("ammo") < 100:
        self.set_recipe("ammo")
    else:
        self.set_recipe("plate")

    if self.can_produce():
        self.produce()
```

---

### 8.9 turret

#### 役割

- 敵を検出し、弾薬を消費して攻撃する。
- 防衛asCodeの最小体験。

#### API

- `scan_enemies() -> list<enemy_info>`
- `attack(enemy_id) -> bool`
- `attack_nearest() -> bool`
- `attack_best(policy) -> bool`
- `ammo_count() -> int`
- `can_attack(enemy_id) -> bool`
- `net_store_get(key) -> value`
- `log(message) -> void`

#### プリセットコード例

```pseudo
export tick(self):
    self.attack_nearest()
```

#### 編集例

```pseudo
export tick(self):
    self.attack_best({
        prefer = ["runner", "wire_cutter", "armored", "nearest"]
    })
```

---

### 8.10 drone_port

#### 役割

- `carrier_drone` の充電。
- ドローンのコード同期。
- ドローンをnetworkに接続。
- 配送ジョブの生成・割当。

#### API

- `charge_docked_drones() -> void`
- `docked_drones() -> list<drone_id>`
- `create_delivery_job(job) -> job_id`
- `pending_jobs() -> list<delivery_job>`
- `dispatch_idle_drones() -> void`
- `stock_count(item_kind) -> int`
- `net_store_get(key) -> value`
- `net_store_set(key, value) -> void`

#### プリセットコード例

```pseudo
export tick(self):
    self.charge_docked_drones()

    if self.stock_count("ammo") > 50:
        self.create_delivery_job({
            item = "ammo",
            amount = 20,
            destination_tag = "frontline"
        })

    self.dispatch_idle_drones()
```

---

## 9. ドローン仕様

### 9.1 MVPドローン: carrier_drone

#### 役割

- アイテムを拾い、目的地に届ける。
- 弾薬補給を担う。

#### パラメータ

```toml
[carrier_drone]
local_cpu_rate = 4
battery_capacity = 100
logic_fuel_capacity = 1000
cargo_capacity = 20
move_speed = 3.0
```

### 9.2 バッテリーとlogic fuel

ドローンには2種類のリソースを持たせる。

| リソース | 消費対象 |
|---|---|
| `battery` | 移動、積載、荷下ろし、物理作業 |
| `logic_fuel` | コード実行、scan、storeアクセス、経路計算 |

MVPでは `logic_fuel` はWasm実行fuelと統合してよい。ただしUI上は「残り判断力」または「logic fuel」として表示する。

### 9.3 接続状態

MVPでは以下の2状態でよい。

| 状態 | 条件 | 効果 |
|---|---|---|
| `docked` | drone_port上にいる | 充電、コード同期、network CPU利用 |
| `offline` | drone_port外で通信不可 | ローカルCPUと残りlogic fuelのみ |

将来、無線塔・通信圏・link_qualityを追加する。

### 9.4 carrier_drone API

- `battery_ratio() -> float`
- `logic_fuel_remaining() -> int`
- `return_to_port() -> bool`
- `claim_delivery_job() -> option<delivery_job>`
- `deliver(job) -> bool`
- `move_to(pos) -> bool`
- `load(item_kind, amount) -> int`
- `unload(item_kind, amount) -> int`
- `cargo_count(item_kind) -> int`
- `idle() -> void`

### 9.5 プリセットコード例

```pseudo
export tick(self):
    if self.battery_ratio() < 0.25:
        self.return_to_port()
        return

    if self.logic_fuel_remaining() < 100:
        self.return_to_port()
        return

    job = self.claim_delivery_job()
    if job != null:
        self.deliver(job)
    else:
        self.idle()
```

---

## 10. 敵仕様: MVP最小セット

MVPの敵は4種類。

| enemy_id | 役割 |
|---|---|
| `grunt` | 基本敵。通常の防衛確認用 |
| `runner` | 高速敵。ターゲット優先度編集を促す |
| `armored` | 高HP敵。集中砲火・弾薬供給を促す |
| `wire_cutter` | 配線・cpu_nodeを狙うXaC固有敵 |

### 10.1 grunt

- 普通の敵。
- コアを目指す。
- タレットの基本挙動で倒せる。

### 10.2 runner

- 移動速度が速い。
- `attack_nearest` だけでは取り逃がしやすい。
- プレイヤーに `attack_best` や優先度編集を促す。

### 10.3 armored

- 低速だがHPが高い。
- MVPでは弾種切替までは必須ではない。
- 複数タレットの集中砲火、弾薬供給が重要。

### 10.4 wire_cutter

- wire、cpu_node、drone_portを優先攻撃する。
- 破壊によりnetwork分断を発生させる。
- XaCの「コードはネットワークに依存するが、ローカルCPUでも遅く動く」を体験させる。

---

## 11. ネットワーク仕様

### 11.1 network の定義

wireで接続された連結成分を1つの `network` とする。

networkは以下を持つ。

- network_id
- 接続ブロック一覧
- CPUプール
- active device数
- network store
- event queue（MVPでは簡易）
- 電力状態（MVPでは詳細な電力シミュレーションは簡略化可能）

### 11.2 CPU供給源

MVPのCPU供給源は以下。

| 供給源 | CPU |
|---|---:|
| `core` | 120 fuel/sec |
| `cpu_node` | 80 fuel/sec |
| `drone_port` | 20 fuel/sec 任意 |

数値は初期調整値であり、実装後にプレイテストで変更する。

### 11.3 ローカルCPU

すべてのプログラム可能ブロックは低速な `local_cpu_rate` を持つ。

例:

| block | local_cpu_rate |
|---|---:|
| `router` | 1 fuel/sec |
| `drill` | 1 fuel/sec |
| `assembler` | 2 fuel/sec |
| `turret` | 3 fuel/sec |
| `drone_port` | 3 fuel/sec |
| `carrier_drone` | 4 fuel/sec |

wireが切れても、ブロックはlocal CPUで遅く動く。

### 11.4 CPU配分式

各networkのCPUは、そのnetwork内の active device に平等配分する。

```txt
effective_cpu_rate(device)
  = device.local_cpu_rate
  + network.cpu_pool / network.active_device_count
```

active device とは、以下のいずれかに該当するもの。

- カスタムコードが有効なブロック
- プリセットでもtick/on_item等のWasm実行が必要なブロック
- ドローン
- drone_port

以下はactive deviceに含めない。

- wall等の非プログラムブロック
- wire
- cpu_node
- デフォルト動作だけでWasmを実行しないconveyor

### 11.5 ネットワーク分割

wire破壊やプレイヤー操作でnetworkが分断された場合:

1. 連結成分を再計算する。
2. 各新networkに新しいnetwork_idを付与、または安定IDを再割当する。
3. CPUプールを再計算する。
4. network storeをどう扱うか決定する。

MVPではnetwork store分断処理を単純化する。

推奨:

- coreを含む側は元storeを保持。
- coreを含まない側は、分断時点のstoreスナップショットを read-only cache として持つ。
- 再接続時にcore側storeを正とする。

将来は競合解決・データブリッジを実装する。

### 11.6 network store

network内で共有できる変数。

MVPでは型付きstoreを完全実装しない。代わりに、Wasm境界で扱える `value` 型を定義する。

#### value型

```wit
variant value {
  none,
  bool(bool),
  s32(s32),
  s64(s64),
  f32(float32),
  f64(float64),
  string(string),
  id(string),
  pos(vec2),
  list(list<value>),
  map(list<kv>),
}

record kv {
  key: string,
  value: value,
}

record vec2 {
  x: s32,
  y: s32,
}
```

#### store API

- `net_store_get(key: string) -> value`
- `net_store_set(key: string, value: value) -> result`
- `net_store_delete(key: string) -> result`

#### fuel cost例

| API | fuel cost |
|---|---:|
| `net_store_get` | `2 + value_size` |
| `net_store_set` | `4 + value_size * 2` |
| `net_store_delete` | `2` |

---

## 12. WebAssembly実行仕様

### 12.1 採用方針

XaCは独自言語を作らない。

プレイヤーコードの実行形式は **WebAssembly** とする。

理由:

- 複数言語で書ける。
- 後から独自言語へ移行するより、最初からWasm ABIを固定した方が安全。
- サンドボックス化しやすい。
- fuelによる実行資源管理と相性がよい。
- ゲーム内CPU資源を実ランタイムのfuelに対応させられる。

### 12.2 推奨ランタイム

MVP実装では **Wasmtime** を推奨する。

理由:

- fuel consumption がある。
- storeごとのfuel設定・取得ができる。
- Component Model / WITを扱える。
- Rust実装のゲームエンジンと相性がよい。

### 12.3 Component Model / WIT

MVPでは、WebAssembly Component Model と WITを使って、ゲームAPIを言語非依存に定義する。

WITは以下を定義する。

- ブロック種別ごとのworld
- 共通型
- host import API
- player module export function

各言語向けSDKはWITから生成する。

MVPで公式サポートするテンプレート言語:

- Rust
- AssemblyScript

MVP外だが互換対象にする言語:

- C / C++
- Zig
- TinyGo
- MoonBit
- WAT直接記述

### 12.4 Capabilityごとのworld

ブロックの物理能力に応じて、異なるWIT worldを使う。

例:

- `world router_behavior`
- `world turret_behavior`
- `world assembler_behavior`
- `world drone_port_behavior`
- `world carrier_drone_behavior`

これにより、`turret` 用Wasmでは攻撃APIが使えるが、`router` 用Wasmでは使えない。

### 12.5 export関数

各Wasm componentは、ブロック種別に応じて以下をexportする。

共通:

```wit
export init: func();
export tick: func();
```

router:

```wit
export on-item: func(item: item) -> router-action;
```

turret:

```wit
export tick: func();
```

carrier_drone:

```wit
export tick: func();
```

MVPでは `tick` と `on-item` を中心にし、イベント駆動は最小限にする。

### 12.6 インスタンス管理

MVPで推奨する方式:

- 同じコードを使う同種ブロックは、コンパイル済みcomponentを共有する。
- 実行時instanceはブロックごとに持つ。
- instanceの永続状態はWasmメモリ内ではなく、必要に応じてhost側の `block_state` に保存することも許容する。

#### 理由

- ブロックごとに独立状態を持てる。
- fork/edit時の反映が分かりやすい。
- 同じcomponentを多数instantiationする場合のコストは、MVPでは最適化対象外。ただしキャッシュする。

### 12.7 fuel割当

各tickで、各active deviceにfuelを付与する。

```txt
fuel_per_tick = effective_cpu_rate / ticks_per_sec
```

例:

```txt
ticks_per_sec = 20
effective_cpu_rate = 100 fuel/sec
fuel_per_tick = 5 fuel/tick
```

Wasm関数呼び出し時、fuelをstoreに設定して実行する。

- fuel内に終了した場合: 成功。
- fuel切れの場合: そのtickの実行は失敗または中断扱い。
- MVPでは「途中中断・再開」は実装しない。
- fuel切れ時は `over_budget` ログを出し、そのブロックの今回の判断をスキップする。

### 12.8 host APIのfuel cost

Wasm命令自体はランタイムfuelを消費する。host API呼び出しは、追加で明示的costを消費させる。

例:

| API | cost |
|---|---:|
| `log` | 1 + message length factor |
| `scan_enemies` | 5 + visible_enemy_count |
| `attack` | 2 |
| `attack_best` | 8 + candidate_count |
| `push_any` | 1 |
| `push` | 1 |
| `set_recipe` | 2 |
| `produce` | 2 |
| `claim_delivery_job` | 5 + pending_jobs |
| `deliver` | 4 |
| `move_to` | 2 |
| `net_store_get` | 2 + value_size |
| `net_store_set` | 4 + value_size * 2 |

### 12.9 ライブラリの実行速度

ライブラリは3種類に分ける。

#### A. プレイヤーが書いた言語内ライブラリ

例:

- Rust crate内の関数
- AssemblyScriptの共通関数
- Cのヘッダ・ライブラリ

これらは最終的にWasmにコンパイルされるため、呼び出したブロックのfuelを消費する。

#### B. XaC SDKラッパー

各言語向けの薄いラッパー。

- WIT import/exportを扱う。
- host APIを呼ぶ。
- SDK自体の処理もWasm内ならfuelを消費する。

#### C. ゲーム組み込みAPI

例:

- `attack_best`
- `find_path`
- `claim_delivery_job`

これはhost側で実装される。Wasm命令数ではなく、明示的なhost API costを消費する。

これにより、便利APIを使えば短いコードで書けるが、CPU資源を消費する。

### 12.10 禁止・制限

プレイヤーWasmには原則としてWASIファイルアクセスを与えない。

MVP制限:

- ホストファイルシステムアクセス禁止
- ネットワークアクセス禁止
- 実時間取得禁止
- 非決定論的乱数禁止
- スレッド禁止
- 外部プロセス起動禁止

乱数が必要な場合はhost API `random_seeded` を使う。

---

## 13. WIT草案

以下は実装開始用の概念的WIT草案である。実装時にwasmtimeの現行仕様に合わせて調整する。

```wit
package xac:mvp;

interface types {
  record vec2 {
    x: s32,
    y: s32,
  }

  type entity-id = string;
  type item-kind = string;
  type enemy-kind = string;

  record item {
    id: entity-id,
    kind: item-kind,
    amount: s32,
  }

  record enemy-info {
    id: entity-id,
    kind: enemy-kind,
    pos: vec2,
    hp: s32,
    distance: f32,
  }

  variant value {
    none,
    bool(bool),
    s32(s32),
    s64(s64),
    f32(float32),
    f64(float64),
    string(string),
    id(entity-id),
    pos(vec2),
    list(list<value>),
    map(list<kv>),
  }

  record kv {
    key: string,
    value: value,
  }

  record delivery-job {
    id: entity-id,
    item: item-kind,
    amount: s32,
    pickup: entity-id,
    dropoff: entity-id,
    priority: s32,
  }
}

interface common-api {
  use types.{value};

  log: func(message: string);
  net-store-get: func(key: string) -> value;
  net-store-set: func(key: string, value: value);
  fuel-remaining: func() -> u64;
}

interface router-api {
  use types.{item};

  push: func(dir: string, item: item) -> bool;
  push-any: func(item: item) -> bool;
  output-available: func(dir: string) -> bool;
}

interface turret-api {
  use types.{enemy-info};

  scan-enemies: func() -> list<enemy-info>;
  attack: func(enemy-id: string) -> bool;
  attack-nearest: func() -> bool;
  attack-best: func(policy: string) -> bool;
  ammo-count: func() -> s32;
}

interface assembler-api {
  set-recipe: func(recipe-id: string) -> bool;
  can-produce: func() -> bool;
  produce: func() -> bool;
  input-count: func(item-kind: string) -> s32;
  output-count: func(item-kind: string) -> s32;
}

interface drone-api {
  use types.{delivery-job};

  battery-ratio: func() -> f32;
  logic-fuel-remaining: func() -> u64;
  return-to-port: func() -> bool;
  claim-delivery-job: func() -> option<delivery-job>;
  deliver: func(job: delivery-job) -> bool;
  idle: func();
}

world router-behavior {
  import common-api;
  import router-api;
  use types.{item};

  export init: func();
  export on-item: func(item: item);
}

world turret-behavior {
  import common-api;
  import turret-api;

  export init: func();
  export tick: func();
}

world assembler-behavior {
  import common-api;
  import assembler-api;

  export init: func();
  export tick: func();
}

world carrier-drone-behavior {
  import common-api;
  import drone-api;

  export init: func();
  export tick: func();
}
```

WIT上ではkebab-caseが自然だが、各言語向けSDKではsnake_case APIを提供する。

例:

- WIT: `scan-enemies`
- Rust SDK: `scan_enemies()`
- AssemblyScript SDK: `scan_enemies()`

---

## 14. コード管理仕様

### 14.1 基本思想

ブロックは以下の組み合わせで定義する。

```txt
physical_base_type + wasm_behavior + config + visual
```

例:

```txt
basic_turret
  base_type: turret
  behavior: builtin/presets/turret/basic.wasm
  source: builtin/presets/turret/basic.rs
  config: builtin/presets/turret/basic.toml
```

### 14.2 built-in preset

ゲーム同梱プリセットはread-only。

プレイヤーが内蔵プリセットを編集しようとした場合、copy-on-writeする。

処理:

1. プレイヤーがプリセットブロックを選択。
2. `Edit` を押す。
3. UIが「内蔵プリセットなのでコピーを作成します」と表示。
4. プロジェクト内に新しいファイルを生成。
5. 選択中ブロックの `behavior_ref` をコピー先に差し替える。
6. エディタを開く。

### 14.3 自作ブロックの編集

自作ブロックを編集するときは2択。

| 操作 | 意味 |
|---|---|
| `edit` | 共有元を直接編集。これを参照する全配置に影響 |
| `fork + edit` | コピーを作成し、選択中ブロックまたは選択範囲だけに適用 |

UIは必ず「このコードを使っている配置数」を表示する。

例:

```txt
behavior: project/blocks/east_wall_turret/behavior.wasm
used_by: 18 blocks

[Edit shared behavior]
[Fork + Edit this block]
[Fork + Edit selected blocks]
```

### 14.4 コード複製

コード複製はファイルコピーではなく、基本は `behavior_ref` の共有で行う。

複数ブロックが同じWasm behaviorを参照できる。

フォーク時のみ新しいbehavior packageを作る。

---

## 15. ファイル配置仕様

### 15.1 原則

ユーザー指定により、ゲーム設定ファイルと共通コードは `~/.config` 配下に置く。

Unix系ではXDG Base Directory Specificationに従い、`$XDG_CONFIG_HOME` が設定されていればそれを使い、未設定または空なら `$HOME/.config` を使う。

### 15.2 config root

```txt
config_root = $XDG_CONFIG_HOME/xac
fallback    = ~/.config/xac
```

### 15.3 ディレクトリ構成

```txt
~/.config/xac/
  settings.toml
  keybindings.toml

  common/
    README.md
    lib/
      targeting/
      logistics/
      pathing/
    blocks/
      smart_router/
      basic_turret_plus/
    templates/
      rust/
      assemblyscript/

  projects/
    default_project/
      project.toml
      resources.toml
      recipes.toml

      blocks/
        east_wall_turret/
          block.toml
          src/
          wit/
          build/
            behavior.wasm
        ammo_router/
          block.toml
          src/
          build/
            behavior.wasm

      blueprints/
      saves_meta/

  cache/
    wasm/
      <hash>.wasm
      <hash>.metadata.toml
```

### 15.4 save data

セーブデータそのものは、サイズが大きくなりやすいため、本来は `$XDG_DATA_HOME/xac/saves` が望ましい。

ただし、MVPでは実装単純化のため以下のどちらかを選択する。

A. すべて `~/.config/xac/projects/<project>/saves` に置く。  
B. code/configは `~/.config/xac`、world saveは `$XDG_DATA_HOME/xac/saves` に置く。

ユーザー指定を優先するならAで開始する。将来の整理を考えるならBにする。

本仕様ではMVPの簡略実装としてAを許容する。

### 15.5 block.toml

```toml
id = "east_wall_turret"
display_name = "East Wall Turret"
base_type = "turret"
language = "rust"
wit_world = "turret-behavior"
source_dir = "src"
wasm = "build/behavior.wasm"

[build]
command = "cargo component build --release"
output = "target/wasm32-wasip1/release/east_wall_turret.wasm"

[placement]
icon = "turret"
category = "defense"

[defaults]
tags = ["frontline", "east_wall"]
```

### 15.6 settings.toml

```toml
[paths]
active_project = "default_project"

[editor]
external_editor = "code"
use_external_editor = false

[wasm]
runtime = "wasmtime"
fuel_enabled = true
max_memory_bytes = 1048576

[gameplay]
ticks_per_second = 20
```

---

## 16. UI仕様

### 16.1 基本レイアウト

画面は常に2分割を基本とする。

```txt
+-----------------------------+-----------------------------+
|                             | File tree / Inspector       |
|        Grid World           |-----------------------------|
|                             | Code Editor                 |
|                             |                             |
+-----------------------------+-----------------------------+
| build palette / status / log                              |
+-----------------------------------------------------------+
```

左:

- グリッド世界
- 建築
- 敵
- ブロック選択
- ネットワーク可視化
- CPU可視化

右:

- ファイルツリー
- コードエディタ
- インスペクタ
- ログ
- ビルドエラー

### 16.2 ブロッククリック

| 操作 | 動作 |
|---|---|
| 左クリック | ブロック選択。挙動ファイルまたはインスペクタを開く |
| `Edit` ボタン | 現在のbehaviorを編集。必要ならcopy-on-write |
| `Fork + Edit` | 自作behaviorをコピーして編集 |
| Ctrl+クリック | ログを開く |
| Alt+クリック | network情報を開く |
| Shift+クリック/ドラッグ | 複数選択 |

### 16.3 ブロックパレット

最初はプリセットから選択する。

カテゴリ例:

- Core
- Power/Network
- Mining
- Logistics
- Production
- Defense
- Drones

各プリセットには以下を表示。

- アイコン
- 名前
- base_type
- 使用behavior
- 必要資源
- CPU性質
- `View Code` / `Edit Copy` ボタン

### 16.4 エディタ

MVPでは内蔵エディタを用意する。

必須機能:

- シンタックスハイライト
- ファイルツリー
- 保存
- ビルド
- ビルドエラー表示
- 使用中ブロック数表示
- behavior_ref表示

外部エディタ起動は任意機能。

### 16.5 ビルドフローUI

1. コード編集。
2. Save。
3. Buildボタン、または自動ビルド。
4. Wasm component生成。
5. WIT world検証。
6. 成功なら選択ブロックにhot reload。
7. 失敗なら以前のWasmを維持し、エラーを表示。

### 16.6 オーバーレイ

MVPで必要なオーバーレイ:

- network overlay: wire連結成分を色分け
- CPU overlay: 各ブロックのeffective_cpu_rate表示
- fuel warning: over_budgetになったブロックに警告
- logistics overlay: droneの配送先・経路
- attack overlay: turretのターゲット

---

## 17. 物流asCode: MVP仕様

### 17.1 目標

MVPの物流asCodeは、複雑な契約システムではなく以下を実現すればよい。

- routerでアイテム分岐を書ける。
- assemblerでレシピ切替を書ける。
- drone_portが配送ジョブを作れる。
- carrier_droneがジョブをclaimして配送できる。
- network storeで簡単な共有状態を使える。

### 17.2 delivery_job

```toml
id = "job_123"
item = "ammo"
amount = 20
pickup = "storage_1"
dropoff = "turret_5"
priority = 50
```

### 17.3 job生成

MVPでは `drone_port` がジョブ生成を担う。

例:

```pseudo
export tick(self):
    ammo = self.stock_count("ammo")
    if ammo > 50:
        self.create_delivery_job({
            item = "ammo",
            amount = 20,
            destination_tag = "frontline",
            priority = 50
        })
```

### 17.4 job取得

carrier_droneは `claim_delivery_job` でジョブを取る。

```pseudo
export tick(self):
    job = self.claim_delivery_job()
    if job != null:
        self.deliver(job)
```

### 17.5 将来のcontract system

将来、各ブロックが以下を宣言するcontract systemに拡張する。

```txt
demand:
  ammo min=30 target=100
supply:
  ammo rate=5
```

MVPでは必須ではない。

---

## 18. 防衛asCode: MVP仕様

### 18.1 基本

turretはプリセットで `attack_nearest` を実行する。

プレイヤーは必要に応じて優先度を編集する。

### 18.2 ターゲット優先度

`attack_best(policy)` はhost組み込みAPI。

policyのMVP表現は単純な文字列または小さな構造でよい。

例:

```pseudo
self.attack_best({ prefer = ["runner", "wire_cutter", "armored", "nearest"] })
```

WITの簡略化のため、MVP実装ではpolicyをJSON文字列として渡してもよい。

### 18.3 wire_cutter対応

`wire_cutter` を優先しないと配線が切られる。

これによりプレイヤーはturretコードを編集する動機を得る。

---

## 19. プリセット仕様

MVPに同梱するプリセット例。

### 19.1 Basic Router

- base_type: `router`
- behavior: `push_any`
- 編集例: item.kindで分岐

### 19.2 Ammo Router

- base_type: `router`
- ammoをfrontline方向へ優先送出

### 19.3 Basic Turret

- base_type: `turret`
- behavior: `attack_nearest`

### 19.4 Priority Turret

- base_type: `turret`
- runner / wire_cutter / armored を優先

### 19.5 Basic Assembler

- base_type: `assembler`
- plateとammoを在庫に応じて作る

### 19.6 Basic Drone Port

- base_type: `drone_port`
- ammo配送jobを作る

### 19.7 Basic Carrier Drone

- unit_type: `carrier_drone`
- battery低下時帰還
- jobがあれば配送

---

## 20. イベントドリブン仕様

### 20.1 MVP方針

イベントドリブンは魅力的だが、MVPでは記述量増加を避けるため `tick` 中心にする。

ただし、routerの `on_item` は必要。

MVP必須export:

- `tick`
- `on_item` only for router

### 20.2 将来拡張

将来、以下を追加する。

- `on_damage`
- `on_network_changed`
- `on_message`
- `on_enemy_seen`
- user-defined event emit/listen

設計上、`net.events.emit` の追加余地を残す。

---

## 21. コーディングスタイル

### 21.1 API命名

ユーザーが見るSDK APIはすべてsnake_case。

例:

- `scan_enemies`
- `attack_best`
- `net_store_get`
- `battery_ratio`
- `logic_fuel_remaining`
- `return_to_port`
- `claim_delivery_job`

WIT内部はkebab-caseでもよいが、SDKはsnake_caseに統一する。

### 21.2 サンプルコードの方針

公式サンプルは短く保つ。

悪い例:

```pseudo
# 長大なforループで全敵をスコアリングする
```

良い例:

```pseudo
export tick(self):
    self.attack_best({ prefer = ["runner", "wire_cutter", "nearest"] })
```

### 21.3 設定中心

MVPの編集体験では、完全なプログラムよりも設定に近いコードを重視する。

例:

```pseudo
export config = {
    target_priority = ["runner", "wire_cutter", "armored", "nearest"]
}
```

ただし、Wasm実行のためには各言語SDK側でこの設定を読み、`tick`に変換するテンプレートを提供する。

---

## 22. Build/Compileパイプライン

### 22.1 手順

1. ユーザーがコードを編集。
2. 言語ごとのbuild commandを実行。
3. Wasm component生成。
4. WIT worldとの互換性検証。
5. hashを計算。
6. `~/.config/xac/cache/wasm/<hash>.wasm` に保存。
7. 対象ブロックのbehavior_refを更新。
8. ゲーム内でhot reload。

### 22.2 build error

ビルド失敗時:

- 現在ゲーム内で動いている旧Wasmは維持。
- 対象ブロックに「build failed」表示。
- エディタにstderrを表示。

### 22.3 runtime error

実行時エラー:

- 対象ブロックのtickをスキップ。
- ログに記録。
- UIに警告。
- 連続エラー時はそのbehaviorを一時停止できる。

### 22.4 over_budget

fuel切れ:

- `over_budget` としてログ。
- そのtickの動作をスキップ。
- UIにCPU不足として表示。
- network CPU追加やコード軽量化の動機にする。

---

## 23. セーブ仕様

### 23.1 セーブ内容

- マップ状態
- ブロック配置
- 各ブロックのbase_type
- behavior_ref
- config_ref
- runtime state
- network状態
- store内容
- 敵状態
- ドローン状態
- tick番号
- RNG seed

### 23.2 コードとの関係

セーブデータはWasm本体を直接埋め込まず、以下を保存する。

- behavior package id
- wasm hash
- source path
- compiled wasm path

ただし、再現性のためにWasm hashが見つからない場合は警告する。

MVPではセーブ内にWasmを同梱するオプションを持ってもよい。

---

## 24. デバッグ機能

MVP必須:

- ブロックごとのログ
- `over_budget` 表示
- build error表示
- runtime error表示
- network overlay
- CPU overlay

MVP任意:

- ブロックごとのfuel使用量履歴
- 配送job一覧
- turretターゲット表示
- drone経路表示

将来:

- タイムライン巻き戻し
- ブレークポイント
- 変数ウォッチ
- debug draw API

---

## 25. プレイヤー初回体験シナリオ

### 25.1 チュートリアル1: 採掘と防衛

1. coreが配置済み。
2. ore資源タイルが近くにある。
3. playerはdrillを置く。
4. conveyorでcoreへ流す。
5. assemblerでammoを作る。
6. turretを置く。
7. grunt waveが来る。
8. turretがプリセットで撃退する。

### 25.2 チュートリアル2: コード編集

1. runner waveが来る。
2. Basic Turretだと取り逃がす。
3. turretをクリック。
4. `Edit`。
5. built-in presetなのでコピー作成。
6. priorityをrunner優先に変更。
7. runnerを撃退できる。

### 25.3 チュートリアル3: CPU

1. 遠くにdrillを置く。
2. wireでcoreにつながないと挙動が遅い。
3. wireでつなぐと処理が速くなる。
4. cpu_nodeを置くとさらに速くなる。
5. CPU overlayで変化を見る。

### 25.4 チュートリアル4: ドローン物流

1. 前線turretのammoが不足する。
2. drone_portとcarrier_droneを作る。
3. ammo配送jobが作られる。
4. droneがammoを届ける。

### 25.5 チュートリアル5: wire_cutter

1. wire_cutterが配線を狙う。
2. wireが切れる。
3. turretやrouterがlocal CPUだけで遅くなる。
4. playerはwireを防衛するか、前線にcpu_nodeを置く。

---

## 26. 実装マイルストーン

### Milestone 1: グリッドと基本ブロック

- 64x64グリッド
- core, wire, drill, conveyor, storage
- item移動
- 基本UI

### Milestone 2: networkとCPU

- wire連結成分
- network_id
- CPUプール
- local_cpu
- active device計算
- CPU overlay

### Milestone 3: Wasmランタイム

- Wasmtime統合
- fuel割当
- WIT world最小実装
- turretまたはrouterのWasm実行
- build/hot reload

### Milestone 4: 生産と防衛

- router
- assembler
- turret
- grunt/runner/armored
- プリセット編集UX

### Milestone 5: ドローン物流

- drone_port
- carrier_drone
- delivery_job
- battery/logic_fuel

### Milestone 6: XaCらしさの完成

- wire_cutter
- network分断
- over_budget UI
- edit / fork + edit
- common code path
- 初回チュートリアル

---

## 27. MVP受け入れ基準

MVPは以下を満たせば「XaCの核が成立した」とみなす。

1. プレイヤーがプリセットだけで採掘・生産・防衛できる。
2. turretのコードまたは設定を編集し、runnerやwire_cutterへの優先度変更ができる。
3. routerのコードを編集し、ammoを任意方向へ流せる。
4. assemblerのコードを編集し、生産レシピを切り替えられる。
5. wireでnetworkが形成され、coreに接続するとブロック処理速度が上がる。
6. cpu_node追加で同じコードの反応速度が改善する。
7. networkに接続機器が増えるとCPUが薄まる。
8. wire破壊によりnetwork分断が起き、ブロックがlocal CPUだけで遅く動く。
9. drone_portとcarrier_droneでammo配送が動く。
10. 内蔵プリセット編集時にcopy-on-writeが起きる。
11. 自作ブロックでは `edit` と `fork + edit` を選べる。
12. プレイヤーコードはWasmとして実行され、fuel不足がゲーム内CPU不足として表現される。
13. ユーザー設定と共通コードが `~/.config/xac` 以下に保存される。

---

## 28. 将来拡張メモ

MVP後に追加する候補。

### 28.1 data_bridge

2つのnetworkをCPU共有せず、変数だけ共有するブロック。

用途:

- core_networkとeast_wall_networkを分ける。
- CPUは独立。
- ammo_requestやthreat_summaryだけ共有。

### 28.2 network_switch

networkの論理接続を切り替える。

用途:

- 敵襲時に防衛networkを隔離。
- CPU節約。
- 被害局所化。

### 28.3 radar / sensor confidence

敵情報や資源情報に信頼度を持たせる。

将来敵:

- jammer
- spoofer
- burrower

### 28.4 combat drone / scout drone / repair drone

RTS性を拡張する。

### 28.5 enemy nest / siege walker

防衛だけでなく、前線攻略と補給線確保をゲームにする。

### 28.6 Blueprint + Code

配置だけでなく、コード・config・network設定を含めた設計図を保存・再配置する。

---

## 29. 参考実装方針

### 29.1 エンジン言語

推奨:

- Rust + Bevy、またはRust独自ECS
- Wasmtimeとの相性を重視

ただし、Unity/Unreal/GodotでもWasm runtimeを統合できるなら可。

### 29.2 determinism

MVPでは完全なロックステップマルチプレイは不要。

ただし、将来のリプレイ・デバッグのために以下を守る。

- fixed tick
- seeded RNG
- Wasmから実時間取得禁止
- Wasmから外部I/O禁止
- host APIの順序を決定論的にする

### 29.3 パフォーマンス

MVPでは全ブロックWasm実行を避ける。

- conveyorはデフォルト物理挙動。
- wire/cpu_nodeはコードなし。
- コード実行対象はactive deviceのみ。
- 同じWasm componentはコンパイルキャッシュを共有。

---

## 30. 開発上の重要な判断

### 30.1 なぜWebAssemblyか

- 多言語対応。
- 変更不能な中核仕様として早期に固定すべき。
- ゲーム内CPU資源をfuelとして表現しやすい。
- サンドボックス化できる。

### 30.2 なぜ独自言語を作らないか

- 学習コストが上がる。
- エディタ補完・外部ツールが弱くなる。
- 後から多言語対応に移行しづらい。

### 30.3 なぜプリセット中心か

- 記述量を減らすため。
- 初心者がコードを書かなくても遊べるため。
- 編集による差分が見えやすいため。

### 30.4 なぜネットワークCPUを平等配分するか

- ルールが説明しやすい。
- ネットワークを大きくしすぎるデメリットが自然に生まれる。
- ネットワーク分割の意味が出る。
- core近くに設備を作るモチベーションが生まれる。

---

## 31. 最終的なMVP像

MVPのXaCでプレイヤーは以下を体験できる。

- Mindustry風にグリッド上へ設備を置く。
- 配線でcoreに接続すると機械が速く動く。
- cpu_nodeを置くと同じコードの反応が改善する。
- turretやrouterをクリックすると、右側エディタで挙動を編集できる。
- built-in presetを編集すると自動的に自分用コピーができる。
- 自作ブロックは共有編集またはfork編集を選べる。
- Wasmにコンパイルできる言語なら、同じWIT APIで挙動を書ける。
- ドローンはバッテリーとlogic fuelを持ち、ネットワーク接続で強化される。
- 敵がwireを切るとネットワークが分断され、基地OSが遅くなる。
- プレイヤーは物流・防衛・CPU・ネットワークを含めて基地を設計する。

このMVPの核は、単なる「コードを書ける工場ゲーム」ではなく、**コード、CPU、ネットワーク、物流、防衛が同じグリッド上で相互作用するRTS**である。

---

## 32. 参考資料

- WebAssembly: https://webassembly.org/
- WebAssembly Component Model / WIT: https://component-model.bytecodealliance.org/design/wit.html
- Wasmtime fuel example: https://github.com/bytecodealliance/wasmtime/blob/main/examples/fuel.rs
- Wasmtime C API store/fuel docs: https://docs.wasmtime.dev/c-api/store_8h.html
- XDG Base Directory Specification: https://specifications.freedesktop.org/basedir/


---

## 33. ADR: 言語・ランタイム選定の決定記録

### 33.1 最終決定

MVPのプレイヤーコード実行形式は **WebAssembly** とする。

プレイヤーは、XaC SDKとWIT worldに対応する限り、複数言語でブロック挙動を書ける。

MVP公式テンプレートはRustとAssemblyScriptから始めるが、ABIは特定言語に依存させない。

### 33.2 検討したが採用しない案

#### TypeScriptそのもの

利点:

- 型と補完が強い。
- ユーザーに馴染みがある。

不採用理由:

- JSランタイム上で機械ごとのCPU速度・fuel消費・サンドボックスをゲームメカニクスとして厳密に扱いづらい。
- 複数言語対応という最終方針に合わない。

#### Luau

利点:

- 小さく、ゲーム内スクリプトとして扱いやすい。
- 記述量が少ない。

不採用理由:

- 多言語対応の最終方針に合わない。
- 後からWasmへ移行すると、コード資産とAPI設計を作り直す可能性が高い。

#### 独自言語

利点:

- ゲームに合わせて最適化できる。

不採用理由:

- 学習コストが高い。
- 外部エディタ、補完、既存言語資産を使いにくい。
- MVPの目的に対して実装範囲が膨らむ。

#### ネイティブx86等

利点:

- 低レベルで高速。
- CPU資源ゲームとしてのロマンがある。

不採用理由:

- サンドボックス化が困難。
- クロスプラットフォーム性が低い。
- 決定論的リプレイやfuel制御が難しい。
- ユーザー生成コードとして安全性を確保しづらい。

### 33.3 Wasmを採用する理由

WebAssemblyは、ネイティブに近い低レベル実行形式でありつつ、サンドボックス・ポータビリティ・多言語コンパイルターゲットを兼ねる。XaCでは、この性質をそのままゲーム内CPU資源とコード管理の中核に使う。

### 33.4 ライブラリに関する決定

プレイヤーが書いたライブラリは、各言語の通常ライブラリとしてWasmにコンパイルされるため、呼び出した機械のfuelを消費する。

ゲーム組み込みAPIはhost側で実装され、明示的なfuel costを消費する。

この区別により、ライブラリ関数の実行速度問題を以下のように解決する。

```txt
ユーザーライブラリ = Wasm命令としてfuel消費
SDKラッパー       = Wasm命令としてfuel消費
host組み込みAPI   = host定義costを消費
```

---

## 34. ADR: 記述量削減のためのUX決定

### 34.1 最終方針

XaCは、プレイヤーに多くのコードを書かせない。

配置できるブロックは最初からプリセットとして表示する。プレイヤーはプリセットを置くだけでゲームを進められる。

### 34.2 プリセット編集

内蔵プリセットには `Edit` ボタンを付ける。

内蔵プリセットはread-onlyであり、編集しようとすると自動で新規ファイルを作成する。

```txt
builtin preset
  ↓ edit
project local copy
  ↓ open editor
player edits
```

### 34.3 自作ブロック編集

自分で作ったブロック + コードを再配置した場合、そのコードは複数配置から共有される可能性がある。

この場合、編集時には以下を選べる。

- `edit`: 共有元を編集し、全配置に反映。
- `fork + edit`: コピーを作り、選択中配置だけに反映。

### 34.4 config優先

可能な限り、長いbehaviorではなく短いconfigを編集する。

例:

```txt
target_priority = runner, wire_cutter, armored, nearest
```

この設定を、各言語SDKまたはプリセットbehaviorが読み込んで実行する。

---

## 35. ADR: MVP外だが保持する世界観・機能コンテキスト

以下はMVPでは実装しないが、XaCの方向性として維持する。

### 35.1 敵タイプ拡張

将来導入候補:

- swarm: 大量雑魚
- raider: 物流狩り
- sapper: 壁や配線に爆弾を仕掛ける
- flyer: 壁を無視する飛行敵
- jammer: network通信を妨害する
- spoofer: 偽信号を出す
- burrower: 地中から侵入する
- siege_walker: 長距離砲撃敵
- nest: 敵前線基地

### 35.2 RTS拡張

将来導入候補:

- scout squad
- assault squad
- repair squad
- combat drone
- outpost core
- 敵拠点攻略
- 補給線を伴う攻撃作戦

### 35.3 Logistics as Code拡張

将来導入候補:

- contract system
- stock target
- priority function
- data_bridge
- networkごとの独立store
- 複数network間の同期ポリシー

MVPではこれらを必要としないが、API・データ構造を後で拡張できるようにする。
