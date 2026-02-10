# Hell Workers - AI Agent Instructions

## Project Overview
"Hell Workers" is a Bevy-based game project. Before starting any task, check these documents:

1. **Project Overview**: [README.md](README.md)
2. **Development Rules**: [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)
3. **Documentation Index**: [docs/README.md](docs/README.md)
4. **Architecture Details**: [docs/architecture.md](docs/architecture.md)

## Key Technical Context
- **Engine**: Bevy 0.18
- **Language**: Rust
- **Critical Mechanism**: ECS Relationships are used for most entity connections
- **Build Target**: Windows (x86_64-pc-windows-gnu)

---

## Development Rules

### 1. Rust-analyzer Diagnostics (STRICT)
After any code change, ensure zero compilation errors:
1. Wait a few seconds for IDE diagnostics after editing
2. If no IDE feedback, run `cargo check` manually
3. Fix errors immediately before any other work
4. Minimize warnings (remove unused imports/variables)

**Completion criteria**: `cargo check` shows "Finished" with no errors.
**Never report completion with errors remaining.**

### 2. No Dead Code
- Do not use `#[allow(dead_code)]` for "future use"
- Do not leave implementations not documented in `docs/`
- If code is unused now, delete it

### 3. Refactoring Rules
- When creating folders or splitting code, always investigate if integration with related files is beneficial

### 4. Debugging Policy
- Avoid asking user to debug manually
- Only request user debugging as last resort when cause cannot be identified
- When debugging, first check implementation once cause is roughly estimated

---

## Task Execution System Conventions

### AssignedTask Structure
When adding new tasks to `AssignedTask` enum:
- Use **struct variants** (not tuple variants)
- Define data structures in `src/systems/soul_ai/execute/task_execution/types.rs`

### Query Aggregation
- Aggregate task queries in `TaskQueries` struct at `src/systems/soul_ai/execute/task_execution/context.rs`
- Do not define individual queries separately

### Execution Context
- Access data (task state, path, inventory) through `TaskExecutionContext`
- Keeps system function arguments minimal

---

## Workflows

### Error Fixing Workflow
1. Run `cargo check`
2. Identify the first error
3. Read the relevant code
4. Fix the error
5. Re-run `cargo check`

### Image Generation Workflow
1. **Generate**: Use `generate_image` with "solid pure magenta background (#FF00FF)"
   - Do NOT specify "transparent background" (AI draws checkerboard pattern)
   - Art style: Refer to `docs/world_lore.md` section 6.2 (Rough Vector Sketch)
   - Key rules: Orthographic projection only (no 3/4 view), loose wobbly lines, textured brush, Tim Burton-esque distorted silhouettes
2. **Convert**: `python scripts/convert_to_png.py "source" "assets/textures/dest.png"`
3. **Verify**: Check PNG signature is `89 50 4e 47 0d 0a 1a 0a`
4. **Use**: Load with `.png` extension in code

### Planning Workflow

#### When to Create a Plan
Create an implementation plan in `docs/plans/` when:
- The task involves significant optimization or refactoring
- Multiple files or systems will be modified
- The implementation approach requires analysis and evaluation
- The user explicitly requests a plan

#### Plan File Management
- **Location**: `docs/plans/` (gitignored - working documents only)
- **Naming**: Use descriptive kebab-case names (e.g., `blueprint-spatial-grid.md`, `taskarea-optimization.md`)
- **Format**: Markdown with clear sections:
  - Problem description
  - Solution approach
  - Expected performance impact
  - Implementation steps
  - Files to modify
  - Verification methods

#### Plan Lifecycle
1. **Creation**: Write detailed plan before implementation
2. **Implementation**: Follow plan steps, updating as needed
3. **Completion**:
   - If successful: Delete plan file or move to archive
   - If relevant for future: Document in `docs/architecture.md` or system-specific docs
   - Plans are temporary working documents, not permanent documentation

#### Why Plans are Gitignored
- Plans are AI working documents for organizing complex tasks
- Completed features should be documented in permanent docs (`docs/*.md`)
- Prevents clutter in version control
- User can manually commit specific plans if needed

### Task Lifecycle
**On task start**: Review `docs/` to understand current specs and implementation status
**On task completion**: Update or create documentation in `docs/` as needed

---

## Useful Commands

```bash
# Check compilation
cargo check

# Build for Windows
cargo build --target x86_64-pc-windows-gnu

# Convert image to transparent PNG
python scripts/convert_to_png.py "source_path" "assets/textures/dest.png"

# Verify PNG signature
head -c 8 "assets/textures/file.png" | od -An -t x1
# Expected: 89 50 4e 47 0d 0a 1a 0a
```

---

## Directory Structure

### Directories to Avoid Reading
These directories contain build artifacts or logs that may cause issues:
- `target/` - Build artifacts
- `dist/` - Distribution files
- `.trunk/` - Trunk cache
- `logs/` - Log files
- `.git/` - Git internal files
- `docs/plans/` - Temporary AI working documents (gitignored)

### Documentation Directories
- `docs/` - Permanent project documentation (version controlled)
- `docs/plans/` - Temporary implementation plans (gitignored, AI working files)

---

## Documentation Structure
Refer to `docs/` for specific system details:
- `soul_ai.md` - Soul (Damned Soul) autonomous behavior
- `familiar_ai.md` - Familiar command and task management
- `tasks.md` - Task system with ECS Relationships
- `logistics.md` - Resource hauling and stockpiles
- `building.md` - Building process and blueprints
- `architecture.md` - Overall structure and dependencies
