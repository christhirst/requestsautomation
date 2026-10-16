# OIM Provisioning Task Automation (`requestsautomation`)

A Rust gRPC service for querying, filtering, sorting, persisting, and remediating failed provisioning tasks in **Oracle Identity Manager / Oracle Identity Governance (OIM 12.2.1.4)** via the [Self Service ProvisioningTask REST API](https://docs.oracle.com/en/middleware/idm/identity-governance/12.2.1.4/oimss/api-provisioningtask.html).

---

## Key Features

1. **Combined OIM REST Query Filtering (`urlfilter`)**:
   - Avoids repetitive filter definitions by expanding multi-key attribute filter lists into their Cartesian `AND` combinations.
   - For example, configuring:
     ```toml
     urlfilter = [
       ["aa::bb::cc", ["AA", "BB", "CC"]],
       ["ggggg", ["aa", "bb", "cc"]]
     ]
     ```
     automatically expands into:
     - `aa::bb::cc+eq+AA AND ggggg+eq+aa`
     - `aa::bb::cc+eq+AA AND ggggg+eq+bb`
     - `aa::bb::cc+eq+AA AND ggggg+eq+cc`
     - `aa::bb::cc+eq+BB AND ggggg+eq+aa`
     - `aa::bb::cc+eq+BB AND ggggg+eq+bb`
     - ...
2. **In-Memory Data Processing & Sorting with Polars**:
   - Converts OIM `ProvisioningTask` JSON fields (`Process Instance.Task Information.Creation Date`, `Objects.Name`, `Process Instance.Task Details.Key`, `Process Definition.Tasks.Task Name`, `Process Instance.Task Information.Target User`) into a Polars `DataFrame`.
   - Filters tasks by Application/Object Name (`filter1` / `filter_object`) and Task Name (`filter2` / `filter_task`).
   - Parses task creation timestamps and sorts tasks chronologically (`Creation Date` ascending by default, or descending via `sort_descending`).
3. **Flexible Output & Storage Modes**:
   - **CSV File (`mode = "file"` or `"csv"`)**: Appends filtered and sorted tasks to a local CSV file (`filelist` in config).
   - **SurrealDB (`mode = "db"` or `"surrealdb"`)**: Writes tasks to the `task` table in SurrealDB.
   - **Direct gRPC Response (`mode = "direct"` or `"grpc"`)**: Returns the tasks directly in the `ListResponse.tasks` field without writing to disk or database.
4. **Task Remediation Actions (`retry` & `manual_complete`)**:
   - Calls `PUT <baseurl><urlput>/{taskid}` (e.g., `/iam/governance/selfservice/api/v1/provtasks/{taskid}`) with `X-Requested-By: rust` and Basic Auth:
     - **`retry`**: sends `{"action": "retry"}`
     - **`manual_complete`**: sends `{"action": "manualComplete"}`
   - Can process tasks sequentially from the CSV file (removing each row as it is processed), from SurrealDB (ordered by oldest creation date first and deleting each record after processing), or directly from a list of `task_ids` passed in the gRPC request.

---

## Project Structure

```text
requestsautomation/
├── build.rs                 # Compiles proto/usr.proto and generates reflection descriptor
├── Cargo.toml               # Dependencies (tonic, prost, polars, reqwest, surrealdb, etc.)
├── config/
│   ├── default.toml         # Base settings (gRPC server bind address, SurrealDB config)
│   ├── development.toml     # OIM REST API credentials, endpoints, filters, timeouts
│   └── production.toml      # Production overrides
├── grpcurl/
│   └── commands.http        # Ready-to-use gRPC request examples
├── proto/
│   └── usr.proto            # Protobuf service & message definitions
└── src/
    ├── main.rs              # Application entrypoint & Tonic gRPC server setup
    ├── config.rs            # Layered TOML + environment configuration loader
    ├── grpcserver.rs        # gRPC User service implementation
    ├── datapolars.rs        # Polars DataFrame construction, filtering & sorting
    ├── dataendpoint.rs      # gRPC action enum to OIM payload mapper
    ├── file.rs              # CSV reading, writing, and queue pop operations
    ├── model.rs             # Serde models for OIM REST API responses
    ├── types.rs             # Action enum ("retry", "manualComplete") & request payload
    ├── error.rs             # Unified CliError and tonic::Status conversions
    ├── db/                  # SurrealDB client and record types
    └── http/                # Reqwest HTTP client, OIM pagination & filter builder
```

---

## Configuration

Configuration is loaded in order from:
1. `config/default.toml`
2. `config/<RUN_MODE>.toml` (defaults to `config/development.toml`)
3. `config/local.toml` (optional, untracked)
4. Environment variables prefixed with `APP_` (and `LOGLEVEL` for logging)

### Example `config/default.toml`
```toml
db = false
loglevel = "DEBUG"

[grpc_server]
port = "50051"
host = "0.0.0.0"

[database]
user = "admin"
password = ""
url = "127.0.0.1:8000"
token = ""
namespace = "test"
database = "test"
```

### Example `config/development.toml`
```toml
[grpc]
grpcport = "http://0.0.0.0:50051"
username = "xelsysadm"
password = "password"
baseurl = "https://oim.example.com:14000"
urlput = "/iam/governance/selfservice/api/v1/provtasks"
urlget = "?q=Process::Instance.Task::Details.Status+eq+Rejected"
urlfilter = [
  ["aa::bb::cc", ["AA", "BB", "CC"]],
  ["ggggg", ["aa", "bb", "cc"]]
]
printmode = false
checkmode = false
filemode = true
entries = 10
filter1 = "AAccount"
filter2 = "Update"
filelist = "path4.csv"
timeout = 30
sleep = 1000
```

---

## Running the Service

```bash
# Run in development mode (default)
cargo run

# Run with custom log level
LOGLEVEL=INFO cargo run
```

The gRPC server listens on `0.0.0.0:50051` by default and exposes **gRPC Server Reflection (v1)** so tools like `grpcurl`, Postman, Bruno, or `solid.grpcfront` can discover services automatically.

---

## gRPC API Usage (`grpcurl`)

### 1. List Available Services & Methods
```bash
grpcurl -plaintext localhost:50051 list
grpcurl -plaintext localhost:50051 list requestsautomation.User
```

### 2. Fetch, Filter & Sort Failed Tasks (`ProvTasksList` / `GenList`)

#### A. Return Tasks Directly via gRPC (`mode: "direct"`)
```bash
grpcurl -plaintext -d '{
  "urlget": "/te?q=123+eq+123 AND ",
  "urlfilter": {
    "aa::bb::cc": { "values": ["AA", "BB", "CC"] },
    "ggggg": { "values": ["aa", "bb", "cc"] }
  },
  "mode": "direct",
  "filter_object": "AAccount",
  "filter_task": "Update",
  "sort_descending": false,
  "limit": 50
}' localhost:50051 requestsautomation.User/ProvTasksList
```

#### B. Save Tasks to CSV File (`mode: "file"`)
```bash
grpcurl -plaintext -d '{
  "urlfilter": {
    "Objects.Name": { "values": ["AAccount"] }
  },
  "mode": "file"
}' localhost:50051 requestsautomation.User/ProvTasksList
```

#### C. Save Tasks to SurrealDB (`mode: "db"`)
Requires `db = true` in `config/default.toml`.
```bash
grpcurl -plaintext -d '{
  "urlfilter": {
    "Objects.Name": { "values": ["AAccount"] }
  },
  "mode": "db"
}' localhost:50051 requestsautomation.User/ProvTasksList
```

---

### 3. Execute Task Actions (`ProvAction`: `retry` or `manual_complete`)

#### A. Retry Tasks from CSV File or SurrealDB
Sends `PUT /iam/governance/selfservice/api/v1/provtasks/{taskid}` with `{"action": "retry"}`:
```bash
grpcurl -plaintext -d '{
  "action": "retry",
  "mode": "file"
}' localhost:50051 requestsautomation.User/ProvAction
```

#### B. Manually Complete Tasks from CSV File or SurrealDB
Sends `PUT /iam/governance/selfservice/api/v1/provtasks/{taskid}` with `{"action": "manualComplete"}`:
```bash
grpcurl -plaintext -d '{
  "action": "manual_complete",
  "mode": "file"
}' localhost:50051 requestsautomation.User/ProvAction
```

#### C. Retry or Manually Complete Specific Task IDs Directly
```bash
grpcurl -plaintext -d '{
  "action": "manual_complete",
  "mode": "direct",
  "task_ids": ["622483", "622484"]
}' localhost:50051 requestsautomation.User/ProvAction
```

---

### 4. Utility Endpoints

- **Check OIM Endpoint Connectivity (`CheckCon`)**:
  ```bash
  grpcurl -plaintext -d '{"id": "default"}' localhost:50051 requestsautomation.User/CheckCon
  ```
- **Reload Configuration & Database Connection (`ConfReload`)**:
  ```bash
  grpcurl -plaintext -d '{"id": "default"}' localhost:50051 requestsautomation.User/ConfReload
  ```
- **Clear Stored Tasks (`DbDelete` - deletes all rows in SurrealDB `task` table or removes the CSV file)**:
  ```bash
  grpcurl -plaintext -d '{"id": "default"}' localhost:50051 requestsautomation.User/DbDelete
  ```
