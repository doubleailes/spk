// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk
use pyo3::prelude::*;

use super::errors::SolverError;
use super::graph::{
    Change, Decision, Graph, Node, Note, RequestPackage, RequestVar, SetOptions, SetPackage,
    SetPackageBuild, SkipPackageNote, StepBack,
};
use super::solution::Solution;
use super::solver::Solver;
use super::validation::Validator;

fn init_submodule_errors(py: &Python, module: &PyModule) -> PyResult<()> {
    module.add("SolverError", py.get_type::<SolverError>())?;
    Ok(())
}

fn init_submodule_graph(module: &PyModule) -> PyResult<()> {
    module.add_class::<Change>()?;
    module.add_class::<Decision>()?;
    module.add_class::<Graph>()?;
    module.add_class::<Node>()?;
    module.add_class::<Note>()?;
    module.add_class::<RequestPackage>()?;
    module.add_class::<RequestVar>()?;
    module.add_class::<SetOptions>()?;
    module.add_class::<SetPackage>()?;
    module.add_class::<SetPackageBuild>()?;
    module.add_class::<SkipPackageNote>()?;
    module.add_class::<StepBack>()?;
    Ok(())
}

fn init_submodule_solution(module: &PyModule) -> PyResult<()> {
    module.add_class::<Solution>()?;
    Ok(())
}

fn init_submodule_solver(module: &PyModule) -> PyResult<()> {
    module.add_class::<Solver>()?;
    Ok(())
}

fn init_submodule_validation(module: &PyModule) -> PyResult<()> {
    module.add_class::<Validator>()?;
    Ok(())
}

pub fn init_module(py: &Python, m: &PyModule) -> PyResult<()> {
    {
        let submod_errors = PyModule::new(*py, "_errors")?;
        init_submodule_errors(py, submod_errors)?;
        m.add_submodule(submod_errors)?;
    }
    {
        let submod_graph = PyModule::new(*py, "graph")?;
        init_submodule_graph(submod_graph)?;
        m.add_submodule(submod_graph)?;
    }
    {
        let submod_solver = PyModule::new(*py, "_solver")?;
        init_submodule_solver(submod_solver)?;
        m.add_submodule(submod_solver)?;
    }
    {
        let submod_solution = PyModule::new(*py, "_solution")?;
        init_submodule_solution(submod_solution)?;
        m.add_submodule(submod_solution)?;
    }
    {
        let submod_validation = PyModule::new(*py, "validation")?;
        init_submodule_validation(submod_validation)?;
        m.add_submodule(submod_validation)?;
    }

    m.add_class::<Graph>()?;
    m.add_class::<Solution>()?;
    m.add_class::<Solver>()?;

    m.add("SolverError", py.get_type::<SolverError>())?;

    Ok(())
}
