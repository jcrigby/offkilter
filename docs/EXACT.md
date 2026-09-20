# Exact surfaces

The `exact-surfaces` branch is the program to make curved faces exact.
This note is the plan and the record of what is done; the sections below
are marked as they land.

## Approach

The polyhedral kernel stays the topological engine. Every face already
carries an analytic surface tag (plane, cylinder; revolved and ruled
surfaces are only smoothing groups so far), and every edge of the solid
lies between two faces. Where both surfaces are analytic, the exact curve
of the edge is implied by the pair: plane ∩ plane is a line, plane ∩
cylinder an ellipse (a circle across the axis, lines along it), cylinder
∩ cylinder a quartic. So exactness is *recovered* from the surface pairs
rather than carried by a second representation:

- a vertex's exact position is the point on all its incident analytic
  surfaces (alternating projection converges in a few steps);
- an edge run (the chain of facet edges between one pair of surfaces) is
  a piece of the pair's curve, and can be sampled anywhere on it: at the
  rulings of a cylinder for facets that stay planar, or finely for
  export;
- a surface's region is the union of its facets, bounded by loops of runs
  to other surfaces.

What the facets are then is a tessellation of exact trimmed faces at the
document's resolution, and a boolean's job is to decide the topology
(which pieces of which faces survive), which the polyhedral booleans do
robustly. Refitting rebuilds the tessellation from the exact curves after
a boolean, so results no longer depend on the resolution of the tools.

## Phases

1. [x] `ok_brep::exact`: curve classification from surface pairs, exact
   vertex positions, edge runs, surface regions, sampling of a run at a
   cylinder's rulings. Read-only over a solid.
2. [ ] STEP export with exact faces: one `ADVANCED_FACE` per cylindrical
   region on a `CYLINDRICAL_SURFACE`, bounds made of runs written as
   `CIRCLE`, `ELLIPSE`, `LINE` or a fine polyline B-spline for quartics,
   vertices at their exact positions, a seam added where a region closes
   round the axis. Round-tripped through the reader.
3. [ ] Refit after booleans: rebuild the facets of every cylindrical
   surface a boolean touched from its exact runs at the document's
   facet angle, and update the neighbouring planar faces' loops to the
   new samples.
4. [ ] Exact curves in drawings and measurements (silhouettes and
   ellipses as curves, not polylines) and DXF arcs.
5. [ ] Revolved surfaces as exact cones and tori (profile line and arc
   about the axis), including fillets along circular edges.
6. [ ] Beyond: general surfaces (sweeps, lofts) as B-spline surfaces,
   which needs a real parametric trimming kernel.
