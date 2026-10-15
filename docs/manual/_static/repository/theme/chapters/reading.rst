Reading a measurement
=====================

A square with side :math:`s` has area :math:`s^2`.
Changing the unit changes the number, but not the measured square.

.. math::

   A = s^2

.. note::

   Compare values only after expressing them in the same unit.

.. list-table:: One square, two units
   :header-rows: 1

   * - Side
     - Area
   * - 1 m
     - 1 square metre
   * - 100 cm
     - 10,000 square centimetres

.. code-block:: odin

   package main

   import "core:fmt"

   area :: proc(side: f64) -> f64 {
       return side * side
   }

   main :: proc() {
       fmt.println(area(2))
   }

.. graphviz::
   :class: graphviz-adaptive
   :caption: Normalize the unit before comparing measurements.
   :alt: A measured value passes through unit conversion before comparison.

   digraph units {
     graph [rankdir=TB, bgcolor="transparent", pad="0.25", ranksep="0.4"];
     node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=12,
           color="#216553", fontcolor="#16382f", fillcolor="#edf5f2", margin="0.2,0.12"];
     edge [color="#216553", arrowsize=0.7];
     value [class="gd-input", label="Measured value", shape=note, fillcolor="#fbfaf3"];
     unit [label="Convert to a common unit"];
     compare [class="gd-output", label="Compare like with like", fillcolor="#eef4fa"];
     value -> unit -> compare;
   }

A useful record
---------------

Write down the instrument, its precision and the conditions of the measurement.
A number without these details hides the uncertainty that matters to the reader.
