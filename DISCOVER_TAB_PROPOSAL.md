# Discover Tab Feature Proposal — RomM Desktop Client

## Executive Summary

The **Discover tab** is a new desktop interface section designed to surface curated game recommendations, new server additions, and interesting content to users without requiring pre-existing knowledge of the server's catalog. It transforms the application from a **game library/server browser** into a **content discovery experience**, analogous to Steam's Discovery Queue or GOG's "What's Popular."

---

## Goals & Objectives

### Primary Objectives
1. **Reduce discoverability barrier** — Help users find games they didn't know existed on the server
2. **Encourage exploration** — Provide low-friction entry points into underexplored platforms or genres
3. **Highlight quality content** — Surface highly-rated, well-reviewed games across the server
4. **Show what's new** — Keep users informed of recent server additions and updates
5. **Personalize without intrusion** — Use user's install history to suggest relevant games (optional, phase 2)

### Success Metrics
- Users spend time exploring Discover (not just Library → Details → Install)
- Users install games they discovered via Discover tabs
- Reduced clicks to find a new game to play
- Increased platform awareness (e.g., users discover PS1 games they didn't know existed)

---

## Feature Architecture

### Core Sections (Horizontal Carousel or Stacked Tabs)

Each section is a **scrollable grid** of game cards, using the existing `AspectRatioLabel` and game card rendering infrastructure from the Server tab. Sections can be:
1. **Horizontally scrollable carousels** (Netflix-style)
2. **Vertically stacked sections** (collapsible)
3. **Tab-based switcher** within Discover (Highlights, New, Popular, By Genre, etc.)

#### **Section 1: Trending This Week**
- **Purpose**: Surface popular/well-reviewed games
- **Data Source**: `GET /api/roms` with `order_by=rating&order_dir=desc&limit=20`
- **Filters Applied**:
  - `rating >= 3.5` (optional minimum quality threshold)
  - Exclude games already installed by user
  - Rotate weekly or on-demand
- **Display**: Grid of 4-6 cards visible, infinite horizontal scroll
- **Metadata**: Show rating badge + genre tags

#### **Section 2: New Additions**
- **Purpose**: Highlight games recently added to the server
- **Data Source**: `GET /api/roms` with `order_by=created_at&order_dir=desc&limit=20`
- **Filters Applied**:
  - Optional: filter by "added in last 7 days", "added in last month"
  - Exclude already installed
- **Display**: Grid of 4-6 cards with "NEW" badge
- **Metadata**: Show addition date relative to now ("Added 2 days ago")

#### **Section 3: Highly Rated**
- **Purpose**: Show consistently well-reviewed games across all time
- **Data Source**: `GET /api/roms` with `order_by=rating&order_dir=desc&limit=30`
- **Filters Applied**:
  - `rating >= 4.0` (higher threshold than Trending)
  - Exclude already installed
  - Optional: only games with 10+ ratings (for rating confidence)
- **Display**: Grid with 4-6 visible cards, horizontal scroll
- **Metadata**: Show rating (e.g., ⭐ 4.5/5), review count

#### **Section 4: Curated by Platform**
- **Purpose**: Introduce user to platforms they may not use often
- **Data Source**: Per-platform curated fetch
  - For each platform: `GET /api/roms?platform_ids={id}&order_by=rating&order_dir=desc&limit=8`
- **Platform Selection Logic**:
  - Show platforms NOT heavily represented in user's Library
  - Rotate a different platform each session (e.g., "Explore PlayStation 2")
  - Or show 2-3 "underused platforms" side-by-side
- **Display**: Horizontal section per platform with 4-5 cards + platform icon/header
- **Metadata**: Platform name, brief description

#### **Section 5: Browse by Genre**
- **Purpose**: Let users filter discovery by interest
- **Data Source**:
  - `GET /api/roms/filters` to load available genres
  - `GET /api/roms?genres={selected}&order_by=rating&order_dir=desc&limit=15` per genre
- **UI Pattern**:
  - Horizontal pill/tag buttons: "Action", "RPG", "Platformer", "Strategy", etc.
  - Selecting a genre shows a carousel of top games in that genre
  - Optional: show multiple genres simultaneously (row per genre)
- **Display**: Clickable genre bar, dynamic grid below
- **Metadata**: Genre count, avg rating for genre

#### **Section 6: Your Recommendations (Phase 2)**
- **Purpose**: Personalized suggestions based on install history
- **Logic**:
  - Extract genres/franchises/companies from user's installed library
  - Fetch similar games: `GET /api/roms?genres={installed_genres}&order_by=rating&order_dir=desc&limit=15`
  - Bias towards highly-rated, non-installed games
- **Display**: "Because you have [Game]..." — show related recommendations
- **Metadata**: Relationship reason ("Similar to Game X you have installed")

---

## Data Sources & API Usage

### API Endpoints Used
| Endpoint | Purpose | Parameters | Frequency |
|----------|---------|------------|-----------|
| `GET /api/roms` | Fetch game lists with sort/filter | `order_by`, `limit`, `offset`, `order_dir` | On tab open, cached per session |
| `GET /api/roms/filters` | Load genre/franchise options | None | Once on app startup, cached |
| `GET /api/platforms` | Platform metadata (names, icons) | None | Once on app startup, cached |
| `GET /api/roms/{id}` | Full game details | ID only | On card click (existing Details view) |

### Caching Strategy
- **Server Payload Cache**: Cache ROM lists for 1 hour per "bucket" (e.g., "top-rated", "new-this-week")
- **Invalidation Triggers**:
  - Manual "Refresh" button in Discover header
  - Weekly automatic refresh (Monday at 00:00 local)
  - After user install/uninstall (small refresh of relevant buckets)
- **Offline Fallback**: Show cached data if server unreachable

---

## UI/UX Design

### Layout Structure

```
┌─────────────────────────────────────────────────────────────┐
│  [Library] [Server] [Discover] [Downloads] [Emulators] ... │  ← Nav bar
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Discover                                        [Refresh]  │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ Trending This Week               [→ More on Server]│   │
│  │                                                     │   │
│  │  [Card] [Card] [Card] [Card] [Card] →              │   │
│  │   ⭐4.8   ⭐4.5    ⭐4.3    ⭐4.7    ...             │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ New on Server (This Week)                           │   │
│  │                                                     │   │
│  │  [NEW] [NEW] [NEW] [NEW] [NEW] →                    │   │
│  │  Card   Card   Card   Card   Card                   │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ Browse by Genre                                     │   │
│  │                                                     │   │
│  │ [Action] [RPG] [Platformer] [Strategy] [Puzzle]    │   │
│  │                                                     │   │
│  │ Genre Results (Action):                             │   │
│  │  [Card] [Card] [Card] [Card] [Card] →              │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ Explore PlayStation 2                               │   │
│  │ (18 games)                                          │   │
│  │                                                     │   │
│  │  [Card] [Card] [Card] [Card] →                      │   │
│  │        Top-rated PS2 titles                         │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ Highly Rated (All-Time)                             │   │
│  │                                                     │   │
│  │  [Card] [Card] [Card] [Card] [Card] →              │   │
│  │  ⭐4.9   ⭐4.8    ⭐4.8    ⭐4.7    ...             │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Key UI Elements

#### Game Card
- Reuse existing `AspectRatioLabel` + game card from Server tab (180px × 250px)
- Show cover art, title on hover
- Badge overlay:
  - **"NEW"** badge (top-left, 7 days) — green/blue
  - **Rating** badge (bottom-right) — ⭐ X.X/5.0
  - **Installed** check (top-right, if applicable) — ✓
- **Interaction**: Click → Opens Game Details (existing UX)

#### Carousel/Scroll Area
- Horizontal scroll with **visible scrollbar** (or hide until hover)
- **Next/Prev arrows** on left/right edges (optional for consistency with Server tab)
- Momentum scrolling on mouse wheel
- Show 4-6 cards at a time (responsive to window width)

#### Section Header
- Title + optional **"Browse All"** link (navigates to Server tab with filter pre-applied)
- Optional **refresh indicator** ("Updated 2 hours ago")
- Collapsible sections (optional, phase 2)

#### Top-Level Controls
- **Refresh Button**: Manual cache invalidation (top-right, near account status)
- **Settings**: Optional per-section toggles (show/hide Trending, etc.)
- **Filter Sidebar** (Optional, Phase 2): Genre/Platform filters that apply across all sections

---

## Implementation Architecture

### Module Organization

#### New Files to Create
1. **`grid_launcher/ui/discover.py`** — Discover page widget and section builders
   - `DiscoverPageWidget` — main page container
   - `DiscoverCarouselSection` — reusable carousel + header
   - Section-specific builders (trending, new, by-genre, etc.)

2. **`grid_launcher/server/discover.py`** — Server-side caching and data fetching
   - `DiscoverCache` — in-memory + optional file-backed cache
   - `fetch_discover_sections()` — parallel API calls for all sections
   - `filter_already_installed()` — exclude user's installed games
   - `rank_unexplored_platforms()` — logic to choose which platform to feature

3. **`tests/test_discover.py`** — Unit tests for caching, filtering, ranking
4. **`tests/test_discover_ui.py`** — UI layout and scroll behavior tests

#### Modified Files
1. **`grid-launcher.py`** (MainWindow)
   - Add Discover tab to `self.stack` in `__init__`
   - Add "Discover" button to nav bar (between Server and Downloads)
   - Update `_switch_page()` to handle Discover index (index=2)
   - Update `self.nav_buttons` list

2. **`grid_launcher/ui/__init__.py`**
   - Export `DiscoverPageWidget`

3. **`grid_launcher/server/orchestrator.py`** (optional)
   - Add discover data to server sync flow (if live refresh desired)

### Class Hierarchy

```
MainWindow
├── DiscoverPageWidget (inherits QWidget)
│   ├── QVBoxLayout (main)
│   │   ├── QHBoxLayout (header: title + refresh button)
│   │   ├── QScrollArea (main content, vertical scroll)
│   │   │   └── QWidget (content container)
│   │   │       └── QVBoxLayout
│   │   │           ├── DiscoverCarouselSection (Trending)
│   │   │           ├── DiscoverCarouselSection (New)
│   │   │           ├── DiscoverFilteredSection (By Genre)
│   │   │           ├── DiscoverCarouselSection (Platform Spotlight)
│   │   │           └── DiscoverCarouselSection (Highly Rated)
│   │   └── LoadingSpinnerWidget (if first-load async)
│   │
│   └── Connections
│       ├── card_clicked → _open_game_details()
│       ├── refresh_clicked → _refresh_discover_cache()
│       └── genre_selected → _render_genre_section()
│
├── DiscoverCache (stateful)
│   ├── cache: dict[str, CacheEntry]
│   │   ├── "trending" → {"games": [...], "timestamp": ...}
│   │   ├── "new" → {...}
│   │   ├── "by_genre:{genre}" → {...}
│   │   └── ...
│   ├── installed_game_keys: set (for filtering)
│   └── Methods
│       ├── get_section(section_id, force_refresh=False)
│       ├── invalidate_section(section_id)
│       ├── is_stale(section_id, ttl=3600)
│       └── export_to_file() / load_from_file()
│
└── Server API integration
    ├── Discover data fetch (parallel requests)
    ├── Platform list fetch
    ├── Genre list fetch
    └── Game details (via existing Details view)
```

### Data Flow

```
User Opens Discover Tab
│
├─→ Check if DiscoverCache is warm (< 1 hour old)
│   ├─→ YES: Render from cache immediately
│   └─→ NO: Show loading spinner, fetch async
│
├─→ Parallel API Calls (in background)
│   ├─→ GET /api/roms?order_by=rating&limit=20
│   ├─→ GET /api/roms?order_by=created_at&limit=20
│   ├─→ GET /api/roms/filters (cache genres)
│   ├─→ GET /api/platforms
│   └─→ For each platform: GET /api/roms?platform_ids=X&limit=8
│
├─→ Filter Results
│   ├─→ Exclude installed games (using self.library_games)
│   ├─→ Rank platforms by "least installed"
│   └─→ Sort by rating/recency as needed
│
├─→ Render Sections
│   ├─→ Build carousel widgets
│   ├─→ Queue cover image loads via existing cover manager
│   └─→ Display
│
└─→ Cache Results
    └─→ Store in DiscoverCache for 1 hour
```

### Threading Model

- **Main Thread**: UI rendering, user interaction
- **Background Worker Thread**: API calls (parallel fetch of 5-10 endpoints)
  - Use existing `grid_launcher.background.workers` pattern (similar to `DetailsCloudRecordsWorker`)
  - Emit signals: `discover_sections_loaded(sections)`, `discover_error(error_msg)`
  - Timeout: 10 seconds (fallback to cached data if slow)

---

## Implementation Phases

### Phase 1: MVP (Foundation)
**Goal**: Core Discover experience with 3 sections

#### Tasks
1. Create `discover.py` UI module with `DiscoverPageWidget`
2. Create `server/discover.py` cache + fetch logic
3. Implement **Trending This Week** section (simple carousel)
4. Implement **New Additions** section (carousel)
5. Implement **By Genre** section (genre pills + dynamic carousel)
6. Add Discover button to nav bar (index=2 in tab order)
7. Wire up card clicks to existing Details view
8. Add basic tests

**Timeline**: ~3-4 days

### Phase 2: Enhancement (Polish & Personalization)
**Goal**: More sections, smarter caching, optional personalization

#### Tasks
1. Add **Explore Platforms** section (unexplored platform spotlight)
2. Add **Highly Rated** section (all-time top games)
3. Implement file-backed cache (survive app restart)
4. Auto-refresh discover data weekly (background task)
5. Add manual refresh button + "Updated X minutes ago" indicator
6. Implement personalized recommendations (if user has 20+ installed games)
7. Add collapsible sections (optional)
8. Comprehensive testing

**Timeline**: ~2-3 days

### Phase 3: Advanced (Interactive & Filtering)
**Goal**: Rich filtering, stats, and engagement features

#### Tasks
1. Add filter sidebar (genre/platform multi-select)
2. Show genre statistics (e.g., "50 RPGs available, 12 installed")
3. Add "See All" links per section (navigate to Server with filters)
4. Implement section preferences (user can hide/reorder sections)
5. Add watchlist feature (mark games as "Interested", separate view)

**Timeline**: ~2-3 days

---

## Technical Considerations

### Performance
- **API Call Volume**: 5-10 parallel requests on first load → consider rate limiting if server is restrictive
- **Cache Size**: ~200 games × 20 fields = ~2MB in memory; negligible
- **Image Loading**: Reuse existing `CoverManager` async loading (already batched and smart)
- **Scrolling**: Use `QScrollArea` with lazy game-card instantiation (similar to Server tab's `_ServerGamePlaceholder`)

### Offline Support
- If server unreachable, show cached data with "Last updated X days ago" indicator
- Optional: show placeholder "Server offline" if no cache available

### Accessibility
- Carousel arrows keyboard-navigable (Tab, Arrow keys)
- Genre pills keyboard-selectable
- Alt text on all cover images
- Color contrast on badges (rating, NEW)

### Theming
- Reuse existing theme colors from `grid_launcher/ui/theme.py`
- Apply same `#panel` styling to section containers
- Consistent card styling (180px × 250px)

### Mobile/Responsive
- Min width: 800px (3 cards visible)
- Max width: unlimited
- Cards scale with window size (using `AspectRatioLabel`)
- Carousels stack vertically on narrow windows (future phase)

---

## UX Flows

### Happy Path: User Discovers & Installs
```
User clicks Discover Tab
  ↓
Sections load (from cache or fetch)
  ↓
User browses Trending section
  ↓
Hovers over interesting card, sees title + rating + genres
  ↓
Clicks card
  ↓
Game Details view opens (existing UX)
  ↓
Reads description, screenshots, reviews
  ↓
Clicks "Install Game"
  ↓
Game queued in Downloads
  ↓
User returns to Discover to browse more
```

### Filter Flow (Phase 2)
```
User sees "Browse by Genre" section
  ↓
Clicks "RPG" pill
  ↓
Section refreshes to show top RPGs
  ↓
User scrolls right to see more
  ↓
Clicks "See All RPGs on Server" link
  ↓
Navigates to Server tab with RPG filter pre-applied
```

### Platform Spotlight Flow
```
Discover loads "Explore PlayStation 2" section
  ↓
User sees 5 top-rated PS2 games they don't have installed
  ↓
Clicks one, reads details
  ↓
Installs it
  ↓
Next session, Discover shows "Explore Dreamcast" (rotate)
```

---

## Success Metrics & Analytics (Phase 3)

Track via lightweight event logging:
- **View Events**: "discover_section_viewed" (which sections, when)
- **Click Events**: "discover_card_clicked" (which game, which section)
- **Install Events**: "game_installed" with `source=discover`
- **Engagement**: Time spent in Discover vs. other tabs

**Goal**: 20%+ of new installs originating from Discover after 2 weeks

---

## Risks & Mitigation

| Risk | Impact | Mitigation |
|------|--------|-----------|
| Server API rate limiting | Discover blocked if too many requests | Implement backoff, cache aggressively, batch requests |
| Slow server response | Poor UX (spinner for 5s+) | 10s timeout, show cached data, async loading |
| User library huge (1000+ games) | Filter slow, memory overhead | Use set-based game key lookups, lazy evaluate |
| Server has few games | Discover sections look empty | Show all available, add "See Also" fallback suggestions |
| User dislikes Discover | Tab gets ignored | Phase 2: section hiding preferences, AB testing |

---

## Appendix: Existing Code Patterns to Reuse

### Game Card Rendering
- Use `AspectRatioLabel` (existing, handles scaling)
- Use `_make_game_card()` method from MainWindow (already handles cover load + click)
- Card size: 180px × 250px (consistent with Server tab)

### Scrollable Carousels
- Reference `grid_launcher/server/view.py` for `_ServerGamePlaceholder` lazy loading pattern
- Use `QScrollArea` + `QGridLayout` (single row)
- Implement scroll-on-hover arrows (optional) or rely on mouse wheel

### Async Data Loading
- Use `QThread` + worker pattern from `grid_launcher/background/workers.py`
- Emit signals: `finished`, `error`
- Connect to main thread slots for UI update

### Caching
- No existing global cache pattern; implement simple dict-based TTL cache
- Optional: serialize to `~/.config/grid-launcher/discover_cache.json` for persistence

### Theming
- Reference `self._theme_color('text', '#f8f8f2')` for dynamic color access
- Use same QSS object names: `#panel`, `.gameCard`, etc.

---

## Next Steps

1. **Discuss & Refine**: Review this proposal with stakeholders
2. **Prioritize**: Decide Phase 1 scope (MVP must-haves)
3. **Create Detailed Spec**: Expand UX flows, finalize API call strategy
4. **Implement Phase 1**: Build MVP over 3-4 days
5. **User Feedback**: Test MVP, gather feedback
6. **Iterate Phases 2-3**: Polish, enhance, measure success
