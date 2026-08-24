# Desktop Design System

> **Canonical source**: This file is the single source of truth for the desktop UI design system.
> The file at `apps/desktop/DESIGN_SYSTEM.md` is a reference copy that points here.

# Notype Desktop Design System

This document outlines the core design tokens, visual guidelines, and component patterns for the Notype/Voice Flow application, specifically focusing on the Desktop UI. It serves as the single source of truth to ensure absolute consistency, high-end aesthetics, and a cohesive user experience across all modules.

---

## 1. Core Design Philosophy

- **Clarity & Restraint**: Avoid dense data displays and visual noise. Use simplified metrics, generous whitespace, and unified visual elements to reduce cognitive load.
- **Friendly & Approachable**: Prefer soft, rounded shapes, playful proportions, and a "Soft Flat Design" aesthetic over rigid, aggressive, or overly technical "AI Coding" styles.
- **Native & Fluid**: Interactions should feel instantaneous and native to the OS. Use subtle transitions and glassmorphism to blend with the desktop environment.
- **Consistency**: Stick strictly to the defined Tailwind utility classes for typography, spacing, borders, and colors.

---

## 2. Foundations

### 2.1 Colors & Theming

Notype relies on a robust Light/Dark mode system managed via Tailwind CSS variables (e.g., `bg-background`, `text-foreground`, `border-border`).

- **Base Colors**: Use neutral tones for the structural UI. `bg-background` for the app base, `bg-card` for modules.
- **Opacity Modifiers**: When applying opacity to HEX CSS variables, use the `color-mix` pattern established in `tailwind.config.js` (e.g., `bg-background/90`).
- **Data Visualization Palette (Charts)**: Avoid highly saturated or primary colors for data curves. Use **muted, deep tones with subtle color hints**:
  - *Light Mode*: Deep Navy (`#1e3a8a`), Forest Green (`#065f46`), Berry Purple (`#701a75`).
  - *Dark Mode*: Ice Blue (`#93c5fd`), Mint Green (`#6ee7b7`), Soft Lavender (`#c084fc`).
  - *Structural*: Separate data colors from UI colors (e.g., use `textMuted` and `grid` colors for axes and chart grids to avoid color bleed).

### 2.2 Typography

Typography relies on specific combinations of font size, weight, and tracking to establish a clear hierarchy.

| Category | Tailwind Class | Usage |
| --- | --- | --- |
| **Hero Title** | `text-[clamp(2.5rem,5vw,4.5rem)] font-bold leading-[1.05] tracking-tight` | Page-level massive titles (e.g., Dashboard welcome). |
| **Page/Section Title** | `text-[1.7rem] font-semibold tracking-[-0.05em] text-foreground` | Top-level headers (Settings pages, Dashboard, History). |
| **Card Title** | `text-lg md:text-xl font-semibold leading-none tracking-tight` | `CardTitle` or distinct section groupings. |
| **Stat Numbers** | `text-xl md:text-2xl font-bold` | Data card numbers, key metrics. |
| **Body Text** | `text-sm leading-7` or `text-base leading-relaxed text-muted-foreground` | Standard paragraphs, `CardDescription`, descriptions. |
| **Metadata / Overline** | `text-[11px] uppercase tracking-[0.2em] font-medium text-muted-foreground` | Decorative labels above sections, auxiliary classification. |
| **Small Label** | `text-xs font-medium text-muted-foreground` | Helper text under inputs, chart scales, badges. |

### 2.3 Border Radius

We use aggressive rounding to achieve the approachable, soft aesthetic. *Note: Avoid using `rounded-sm`, `rounded-md`, or `rounded-lg`.*

| Token | Tailwind Class | Usage |
| --- | --- | --- |
| **Full** | `rounded-full` | `Button`, `Switch`, Tabs (`MultiSwitch`), Pills, Badges, icon backgrounds. |
| **Small** | `rounded-2xl` (16px) | `Input`, `Textarea`, `Select`, `HotkeyInput`, inner small cards, chart containers. |
| **Medium** | `rounded-[1.5rem]` / `rounded-3xl` (24px) | Standard `Card`, independent module panels (e.g., Settings blocks). |
| **Large** | `rounded-[2.5rem]` (40px) | Large Hero sections, outermost page background containers. |

### 2.4 Spacing & Grid System

We use a strict **4px grid** for padding, margins, and sizing, utilizing Tailwind's spacing scale (`1` = `4px`).

- **Page Layout**: `mx-auto max-w-6xl p-10` (Unified across Dashboard, Settings, History).
- **Card Inner Padding**: `p-5 md:p-6` (Note: Card Content/Footer usually remove top padding `pt-0` when following a Header).
- **Tight / Internal**: `p-3` (12px), `gap-1`, `gap-2` for compact elements within cards.

---

## 3. Visual Styles & Effects

### 3.1 Glassmorphism & Translucency

- **Floating Elements**: Use `bg-background/95 backdrop-blur-xl` on sticky headers, floating menus, or stacked cards over complex backgrounds.
- **Translucent Cards**: When placing cards over a background image (e.g., Dashboard hero stats), use `bg-background/40 backdrop-blur-xl` combined with semi-transparent borders (`border-border/40`) to let the background bleed through elegantly.

### 3.2 Shadows & Glows

- **Restraint**: Use shadows sparingly. Prefer soft, broad glows over harsh, offset drop shadows.
- **Ambient Glows**: Use absolute positioned divs with `blur-3xl`, `rounded-full`, and low opacity (e.g., `opacity-20`) to create ambient color spots behind cards or charts.
- **Elevation**: Use `shadow-sm` on interactive translucent cards to provide a slight lift.

### 3.3 Background Images

- When using rich images as section backgrounds, ensure text readability by overlaying a gradient mask (e.g., `bg-gradient-to-b from-background/0 via-background/20 to-background/80`).
- Always provide a solid fallback color underneath images (`bg-secondary/10` or `bg-black/20`) to prevent jarring visual gaps during load or extreme aspect ratio scaling.
- Control object fit strictly. For hero headers, use `min-h-full min-w-full object-cover` with absolute centering to prevent white gaps on extreme window resizing.

---

## 4. Components & Patterns

### 4.1 Interactive Elements

- **Default Button**: `h-10 px-5 py-2 rounded-full`
- **Small Button (`sm`)**: `h-9 px-4 rounded-full`
- **Icon Button**: `h-10 w-10 rounded-full flex items-center justify-center`
- **Inputs & Selects**: `h-10 px-4 rounded-2xl`
- **Textareas**: `px-4 py-3 rounded-2xl`
- **Tabs / MultiSwitch**: `h-10 px-4 py-1.5 rounded-full`

### 4.2 Form Validation & Error States

- **Local Validation**: Form validations for settings inputs are performed locally within the component, setting error states directly rather than using a centralized form validation library.
- **Error Styling**:
  - Inputs: `border-destructive focus-visible:ring-1 focus-visible:ring-destructive`
  - Text: `text-destructive text-xs`

### 4.3 Responsive Rules

- Mobile/small windows default to smaller sizes and radii (e.g., `rounded-2xl`, `p-3`, `text-xl`).
- Use the `md:` prefix to apply standard desktop sizes (e.g., `md:rounded-3xl`, `md:p-6`, `md:text-2xl`).
- Desktop app window minimum size is strictly constrained to `960x720`.

---

## 5. Illustration System

Illustrations in Notype Desktop are **product illustrations**, not marketing hero art. Their job is to explain intent instantly, reduce cognitive friction, and add warmth without competing with the UI.

### 5.1 Style Definition: Soft Flat Design

- **Fresh and Breathable**: Base the illustration on a fresh, low-saturation pastel palette (e.g., Mint Green, Baby Blue, Soft Lavender, Blush Pink, Butter Yellow).
- **Friendly and Soft**: 100% rounded geometry. Circles, pills, and squircle-like blobs. **Absolutely no sharp angles.** Use `stroke-linecap="round"` and `stroke-linejoin="round"`.
- **Flat but Layered**: True flat design. Depth is achieved only through opacity overlap or simple 2D layering, not drop shadows.
- **Completely Safe**: Use soft tonal strokes (e.g., a darker blue stroke over a light blue fill). Do NOT use `#000000` or `#1A1A1A` black outlines.

### 5.2 Complexity Budget

The target is "understandable in 3 seconds." Each illustration should generally contain:
- 1 primary object (e.g., a microphone, a shield).
- 1 secondary support object or structural layer.
- 2-4 small decorative accents at most (e.g., soft colored dots or simple rounded sparkles).
- *Avoid*: Multiple competing focal points, dense scene-building, complex perspective.

### 5.3 Technical Implementation & Formats

- **Format**: Prefer **WebP** (`.webp`) or heavily optimized **PNG** (`.png`) for complex, rich, or large-scale illustrations (like Dashboard backgrounds) to maintain absolute cross-platform compatibility and minimal bundle size.
- **Compression**: Always pass assets through `cwebp` or `pngquant` before committing. Aim for < 500KB for large hero images, and < 50KB for small spot illustrations.
- **SVG**: If using SVG, keep the structure clean, hand-editable, and avoid unnecessary groups, masks, filters, and path fragmentation.

### 5.4 Current Product Mapping

For the current onboarding and cloud service flows, the semantic center of each illustration is:
- **Permissions**: safety, access confirmation, trust (Shield & Checkmark)
- **Language**: input/output conversion, understanding, dialogue (Overlapping Bubbles)
- **Model**: intelligence core, processing (Soft AI Node/Core)
- **Hotkey**: trigger, speed, immediate action (Rounded Keycap)
- **Practice**: speaking, capture, feedback loop (Microphone & Waves)
- **Done**: completion, confidence, small celebration (Rounded Star Badge)
- **Cloud STT**: speech becomes text through a cloud service (Cloud & Waveform)
- **Cloud Polish**: text refinement, cleanup, quality uplift (Magic Wand)
