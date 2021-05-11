// Snowcap: Synthesizing Network-Wide Configuration Updates
// Copyright (C) 2021  Tibor Schneider
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with this program; if not, write to the Free Software Foundation, Inc.,
// 51 Franklin Street, Fifth Floor, Boston, MA 02110-1301 USA.

use crate::utils::*;
use serde_json;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;

use plotly::common::{Marker, Mode};
use plotly::histogram::{Bins, HistNorm};
use plotly::layout::{BarMode, Layout};
use plotly::{Histogram, NamedColor, Plot, Scatter};

pub fn show(
    filename: String,
    num_bins: usize,
    output: Option<String>,
) -> Result<(), Box<dyn Error>> {
    let file = File::open(&filename)?;
    let reader = BufReader::new(file);
    let serde_result: Result<Vec<CostResult>, _> = serde_json::from_reader(reader);
    match serde_result {
        Ok(data) => return show_cost(data, num_bins, output.as_ref()),
        Err(_) => {}
    }

    // data is not a cost result. Try with ProblemSeverityResult
    let file = File::open(filename)?;
    let reader = BufReader::new(file);
    let serde_result: Result<Vec<ProblemSeverityResult>, _> = serde_json::from_reader(reader);
    match serde_result {
        Ok(data) => return show_problem_severity(data, num_bins, output.as_ref()),
        Err(_) => {}
    }
    panic!("Cannot read the json file!");
}

fn show_problem_severity(
    data: Vec<ProblemSeverityResult>,
    num_bins: usize,
    output: Option<&String>,
) -> Result<(), Box<dyn Error>> {
    let do_plot = output.is_none();

    let mut html_page: String = get_html_header();

    html_page.push_str("<h1>Severity Measure Description</h1>\n");
    html_page.push_str("<h2>Total Severity</h2>\n");
    html_page.push_str(
        "<p>
    This measure is computed by counting the unreachable routes (from router to prefix) during each
    step of the reconfiguration, and normalizing this number by dividing it by the product (number
    of routers * number of prefixes * number of reconfiguration steps). If, during every single step
    of the reconfiguration, every router cannot reach any prefix, then the total severity is 1.
</p>\n",
    );
    html_page.push_str("<h2>Per-Step Severity</h2>\n");
    html_page.push_str(
        "<p>
    This measure is computed by counting the number of unreachable routes (from router to prefix) at
    a single step of the reconfiguration. It is not averaged over the entire duration of the
    reconfiguration. If, at one point during the reconfiguration process, every router cannot reach
    any prefix, then the step severity is 1.
</p>\n",
    );

    for (i, d) in data.into_iter().enumerate() {
        html_page.push_str(&format!(
            "\n<br />\n<br />\n<br />\n<h2> Network {}: {}</h2>\n<br />\n",
            i + 1,
            d.scenario.file.split("/").last().unwrap(),
        ));
        html_page.push_str(&d.scenario.html_description());
        html_page.push_str("<h3>Random Permutations</h3>\n");
        html_page.push_str(&d.random_permutations.summary_to_html());
        html_page.push_str("<h3>Random router order</h3>\n");
        html_page.push_str(&d.random_router_order.summary_to_html());

        html_page.push_str("<h3>Comparison: Total Severity</h3>\n");
        html_page.push_str("<div style=\"height: 70%; width: 100%;\">\n");
        html_page.push_str(&plot_problem_probability_histogram(
            &d.random_permutations.total_severity,
            &d.random_router_order.total_severity,
            num_bins,
            do_plot,
            1,
        ));
        html_page.push_str("</div>\n");
        html_page.push_str("<h3>Comparison: Per-Step Severity</h3>\n");
        html_page.push_str("<div style=\"height: 70%; width: 100%;\">\n");
        html_page.push_str(&plot_problem_probability_histogram(
            &d.random_permutations.per_step_severity,
            &d.random_router_order.per_step_severity,
            num_bins,
            do_plot,
            37,
        ));
        html_page.push_str("</div>\n<br />\n");
    }

    html_page.push_str("</div>\n</body>\n</html>");

    if let Some(output_file) = output {
        assert!(output_file.ends_with(".html"));
        std::fs::write(output_file, &html_page)?;
    }

    Ok(())
}

pub fn plot_problem_probability_histogram(
    data_random_permutations: &StatisticsResult<f64>,
    data_random_router_order: &StatisticsResult<f64>,
    num_bins: usize,
    show: bool,
    step_by: usize,
) -> String {
    let permut_max_val = data_random_permutations.max;
    let permut_min_val = data_random_permutations.min;
    let router_max_val = data_random_router_order.max;
    let router_min_val = data_random_router_order.min;
    let max_val = if permut_max_val > router_max_val {
        permut_max_val
    } else {
        router_max_val
    };
    let min_val = if permut_min_val < router_min_val {
        permut_min_val
    } else {
        router_min_val
    };

    let size = (max_val - min_val) / (num_bins as f64);

    let values_random_permutations: Vec<_> = data_random_permutations
        .values
        .iter()
        .step_by(step_by)
        .cloned()
        .collect();
    let values_random_router_order: Vec<_> = data_random_router_order
        .values
        .iter()
        .step_by(step_by)
        .cloned()
        .collect();

    let permut_trace = Histogram::new(values_random_permutations)
        .name("random permutations")
        .opacity(0.4)
        .auto_bin_x(false)
        .x_bins(Bins::new(min_val, max_val, size))
        .hist_norm(HistNorm::Probability)
        .marker(Marker::new().color(NamedColor::Red));

    let router_trace = Histogram::new(values_random_router_order)
        .name("random router order")
        .opacity(0.4)
        .auto_bin_x(false)
        .x_bins(Bins::new(min_val, max_val, size))
        .hist_norm(HistNorm::Probability)
        .marker(Marker::new().color(NamedColor::Blue));

    let mut plot = Plot::new();
    plot.add_trace(permut_trace);
    plot.add_trace(router_trace);

    let layout = Layout::new().bar_mode(BarMode::Overlay);
    plot.set_layout(layout);

    if show {
        plot.show();
    }

    plot.to_inline_html(None)
}

fn show_cost(
    data: Vec<CostResult>,
    num_bins: usize,
    output: Option<&String>,
) -> Result<(), Box<dyn Error>> {
    let do_plot = output.is_none();

    let mut html_page: String = get_html_header();

    for (i, d) in data.into_iter().enumerate() {
        html_page.push_str(&format!(
            "\n<br />\n<br />\n<br />\n<h2> Network {}: {}</h2>\n<br />\n",
            i + 1,
            d.scenario.file.split("/").last().unwrap(),
        ));
        html_page.push_str(&d.scenario.html_description());
        html_page.push_str(&d.summary_to_html());
        html_page.push_str("<div style=\"height: 70%; width: 100%;\">\n");
        html_page.push_str(&plot_cost_histogram(d, num_bins, do_plot));
        html_page.push_str("</div>\n");
    }

    html_page.push_str("</div>\n</body>\n</html>");

    if let Some(output_file) = output {
        assert!(output_file.ends_with(".html"));
        std::fs::write(output_file, &html_page)?;
    }
    Ok(())
}

pub fn plot_cost_histogram(mut data: CostResult, num_bins: usize, show: bool) -> String {
    let max_val = vec![data.random_permutations.cost.max, data.optimizer.cost.max]
        .into_iter()
        .fold(0. / 0., f64::max);

    let min_val = vec![
        data.random_permutations.cost.max,
        data.optimizer.cost.max,
        data.ideal_cost,
    ]
    .into_iter()
    .fold(1. / 0., f64::min);

    assert!(!max_val.is_nan());
    assert!(!min_val.is_nan());
    assert!(max_val >= min_val);

    let size = (max_val - min_val) / (num_bins as f64);

    data.random_permutations.cost.values[0] += size / 10.0;
    data.optimizer.cost.values[0] += size / 10.0;

    let trace_random = Histogram::new(data.random_permutations.cost.values)
        .name("Random Permutations")
        .opacity(0.4)
        .auto_bin_x(false)
        .x_bins(Bins::new(min_val, max_val, size))
        .hist_norm(HistNorm::Probability)
        .marker(Marker::new().color(NamedColor::Red));
    let trace_optimizer = Histogram::new(data.optimizer.cost.values)
        .name("Tree Optimizer")
        .opacity(0.4)
        .auto_bin_x(false)
        .x_bins(Bins::new(min_val, max_val, size))
        .hist_norm(HistNorm::Probability)
        .marker(Marker::new().color(NamedColor::Blue));
    let trace_best = Scatter::new(vec![data.ideal_cost, data.ideal_cost], vec![0.0, 0.2])
        .name("Ideal Cost")
        .mode(Mode::Lines)
        .marker(Marker::new().color(NamedColor::Black));

    let mut plot = Plot::new();
    plot.add_trace(trace_random);
    plot.add_trace(trace_optimizer);
    plot.add_trace(trace_best);

    let layout = Layout::new().bar_mode(BarMode::Overlay);
    plot.set_layout(layout);

    if show {
        plot.show();
    }

    plot.to_inline_html(None)
}

fn get_html_header() -> String {
    let mut html_page: String = String::new();
    html_page.push_str(
        "
<html>
<head></head>
<style>
.content {
  max-width: 1000px;
  margin: auto;
  font-family: \"Lucida Sans Unicode\", \"Lucida Grande\", sans-serif;
}
</style>
<body>
    <script src=\"https://cdnjs.cloudflare.com/ajax/libs/mathjax/2.7.5/MathJax.js?config=TeX-AMS-MML_SVG\"></script>
    <script type=\"text/javascript\">if (window.MathJax) {MathJax.Hub.Config({SVG: {font: \"STIX-Web\"}});}</script>
    <script type=\"text/javascript\">window.PlotlyConfig = {MathJaxConfig: 'local'};</script>
    <script src=\"https://cdn.plot.ly/plotly-1.54.6.min.js\"></script>
<div class=\"content\">
",
    );
    html_page
}
