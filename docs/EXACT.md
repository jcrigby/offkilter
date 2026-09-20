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
2. [x] STEP export with exact faces: one `ADVANCED_FACE` per cylindrical
   region on a `CYLINDRICAL_SURFACE`, bounds made of runs written as
   `CIRCLE`, `ELLIPSE`, `LINE` or a fine polyline B-spline for quartics,
   vertices at their exact positions, a seam added where a region closes
   round the axis. Round-tripped through the reader, which now shares
   each cylindrical face's strip columns with the edges on it so the
   faces on both sides of an edge use the same points.
3. [x] Refit after booleans (`exact::refit`, run at the end of every
   boolean on the overlap of the operands' boxes): every vertex moves to
   the point on all of its analytic surfaces, held to the ruling it was
   made on where two of its facets on a cylinder meet along one, so
   vertices keep their order along a curve and two corners that are one
   exact point (a ruling of each cylinder through the same point of the
   curve) meet and are welded. A vertex of a run that would still change
   places with a neighbour is put onto it. Facets that bend by that are
   split into planar triangles, Delaunay-flipped so no triangle is made
   of three consecutive vertices along a nearly straight edge (its plane
   would lie nowhere near the surface, and the next boolean would strip
   it as a spike and tear the mesh). The boolean's intersection vertices
   are already at both bodies' rulings, so this is the rebuild of the
   touched cylinders from their exact runs at the document's resolution.
   Anything that fails to close afterwards leaves the boolean's own
   result in place.
4. [x] Exact curves in drawings and measurements: a drawing view keeps
   its hidden-line work on the facet chords but returns every circle or
   ellipse edge as arcs of the ellipse it projects to (`ViewLines`
   `visible_arcs` and `hidden_arcs`, joined from the chords' visible and
   hidden pieces; the two rims of a hole seen along it are one circle,
   a rim seen edge-on one line). The sheet draws them as SVG arcs, the
   DXF (now R2000) as `CIRCLE`, `ARC` and `ELLIPSE` entities, and the
   measure tool reports an edge's length along its curve
   (`exact::run_length`). Quartics stay polylines.
5. [~] Revolved surfaces as exact cones, tori and spheres. Done: the
   surface tags (`Surface::Cone`, `Torus`, `Sphere`) from a revolve's
   oblique lines and arcs and from the rolling-ball corner patch, their
   projections, the circles where a plane across the axis or a coaxial
   surface of revolution meets them (`exact::run_curve`, which takes a
   point of the run to tell which of a pair's circles it is on), cone
   rulings held like cylinder rulings, refit of booleans on such bodies,
   and so arcs in drawings and exact rim lengths for turned parts. To
   do: the STEP writer and reader for `CONICAL_SURFACE`,
   `TOROIDAL_SURFACE` and `SPHERICAL_SURFACE` faces, and fillets along
   circular edges as tori (chain blends still carry `Surface::Ruled`).
6. [ ] Beyond: general surfaces (sweeps, lofts) as B-spline surfaces,
   which needs a real parametric trimming kernel.
