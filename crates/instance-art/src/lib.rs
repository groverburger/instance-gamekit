//! Pixel art for agents that write GameMaker 8.1 games in code and judge the result
//! from pictures and numbers.
//!
//! * [`art`] — the `.art` text format (see README.md): colour ramps, pixel grids,
//!   shapes, noise, shading, outlines and frames; parsed and rendered to canvases.
//! * [`game`] — sprites and backgrounds of a game, where they are used, and
//!   [`game::apply`], which writes `.art` resources into a project.
//! * [`sheet`] — contact sheets: every sprite zoomed in against the game's backdrop,
//!   light and dark, as a silhouette with its mask and origin, at real size, with
//!   frame-to-frame motion and background seams.
//! * [`check`] — contrast against the backdrop, palette, stray pixels, soft edges,
//!   outline and pixel-size consistency, stale bounding boxes, origins that make
//!   flipping or rotating jump, wrap seams, and `.art` files that were not built.
//!
//! Binary: `gmk-art` (`build`, `sheet`, `check`).

pub mod art;
pub mod canvas;
pub mod check;
pub mod color;
pub mod expr;
pub mod font;
pub mod game;
pub mod sheet;
