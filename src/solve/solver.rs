// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk
use pyo3::{create_exception, prelude::*};
use std::sync::{Arc, Mutex, RwLock};

use crate::{
    api::{self, Build, CompatRule, OptionMap, Request},
    solve::graph::{ChangeBaseT, GraphError, StepBack},
};

use super::{
    errors::{self, SolverError},
    graph::{
        self, Changes, Decision, Graph, Node, NoteEnum, RequestPackage, RequestVar,
        SkipPackageNote, State, DEAD_STATE,
    },
    package_iterator::{
        BuildIterator, EmptyBuildIterator, PackageIterator, RepositoryPackageIterator,
        SortedBuildIterator,
    },
    solution::{PackageSource, Solution},
    validation::{self, BinaryOnlyValidator, ValidatorT, Validators},
};

create_exception!(errors, SolverFailedError, SolverError);

#[pyclass]
pub struct Solver {
    repos: Vec<PyObject>,
    initial_state_builders: Vec<Changes>,
    validators: Vec<Validators>,
    last_graph: Arc<RwLock<Graph>>,
}

// Methods not exposed to Python
impl Solver {
    fn get_iterator(&self, node: &mut Node, package_name: &str) -> Box<dyn PackageIterator> {
        if let Some(iterator) = node.get_iterator(package_name) {
            return iterator;
        }
        let iterator = self.make_iterator(package_name);
        node.set_iterator(package_name, iterator.clone());
        iterator
    }

    fn make_iterator(&self, package_name: &str) -> Box<dyn PackageIterator> {
        assert!(!self.repos.is_empty());
        Box::new(RepositoryPackageIterator::new(
            package_name.to_owned(),
            self.repos.clone(),
        ))
    }

    fn resolve_new_build(&self, _spec: &api::Spec, _state: &State) -> crate::Result<Solution> {
        todo!()
    }

    fn step_state(&self, node: &mut Node) -> crate::Result<Option<Decision>> {
        let mut notes = Vec::<NoteEnum>::new();
        let request = if let Some(request) = node.state.get_next_request()? {
            request
        } else {
            return Ok(None);
        };

        let mut iterator = self.get_iterator(node, request.pkg.name());
        while let Some((pkg, builds)) = iterator.next()? {
            let mut compat = request.is_version_applicable(&pkg.version);
            if !&compat {
                iterator.set_builds(
                    &pkg.version,
                    Arc::new(Mutex::new(EmptyBuildIterator::new())),
                );
                notes.push(NoteEnum::SkipPackageNote(SkipPackageNote::new(
                    pkg.clone(),
                    compat,
                )));
                continue;
            }

            // XXX is this isinstance possible to be true?
            /* if !isinstance(builds, SortedBuildIterator): */
            let builds = Arc::new(Mutex::new(SortedBuildIterator::new(
                node.state.get_option_map(),
                builds.clone(),
            )?));
            iterator.set_builds(&pkg.version, builds.clone());

            while let Some((spec, repo)) = builds.lock().unwrap().next()? {
                let mut spec = spec;
                let build_from_source = spec.pkg.build == Some(Build::Source)
                    && request.pkg.build != Some(Build::Source);
                if build_from_source {
                    if let PackageSource::Spec(spec) = repo {
                        notes.push(NoteEnum::SkipPackageNote(
                            SkipPackageNote::new_from_message(
                                spec.pkg.clone(),
                                "cannot build embedded source package",
                            ),
                        ));
                        continue;
                    }

                    // FIXME: This should only match `PackageNotFoundError`
                    match repo.read_spec(&spec.pkg.with_build(None)) {
                        Ok(s) => spec = s,
                        Err(_) => {
                            notes.push(NoteEnum::SkipPackageNote(
                                SkipPackageNote::new_from_message(
                                    spec.pkg,
                                    "cannot build from source, version spec not available",
                                ),
                            ));
                            continue;
                        }
                    }
                }

                compat = self.validate(&node.state, &spec)?;
                if !&compat {
                    notes.push(NoteEnum::SkipPackageNote(SkipPackageNote::new(
                        spec.pkg, compat,
                    )));
                    continue;
                }

                let mut decision = if build_from_source {
                    match self.resolve_new_build(&spec, &node.state) {
                        Ok(build_env) => Decision::build_package(&spec, &repo, &build_env),

                        // FIXME: This should only match `SolverError`
                        Err(err) => {
                            notes.push(NoteEnum::SkipPackageNote(
                                SkipPackageNote::new_from_message(
                                    spec.pkg,
                                    &format!("cannot resolve build env: {:?}", err),
                                ),
                            ));
                            continue;
                        }
                    }
                } else {
                    Decision::resolve_package(&spec, repo)
                };
                decision.add_notes(notes.iter());
                return Ok(Some(decision));
            }
        }

        Err(errors::Error::OutOfOptions(errors::OutOfOptions { request, notes }).into())
    }
}

#[derive(FromPyObject)]
pub enum RequestEnum {
    Ident(api::Ident),
    Request(api::Request),
    String(String),
}

#[pymethods]
impl Solver {
    #[new]
    fn new() -> Self {
        Solver {
            repos: Vec::default(),
            initial_state_builders: Vec::default(),
            validators: validation::default_validators(),
            last_graph: Arc::new(RwLock::new(Graph::new())),
        }
    }

    /// Add a repository where the solver can get packages.
    pub fn add_repository(&mut self, repo: PyObject) {
        self.repos.push(repo);
    }

    /// Add a request to this solver.
    pub fn add_request(&mut self, request: RequestEnum) -> PyResult<()> {
        let mut request = request;
        let request = loop {
            match request {
                RequestEnum::Ident(r) => {
                    request = RequestEnum::String(r.to_string());
                    continue;
                }
                RequestEnum::String(request) => {
                    let mut request = serde_yaml::from_str::<api::PkgRequest>(&format!(
                        "{{\"pkg\": {}}}",
                        request
                    ))
                    .map_err(|err| {
                        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(err.to_string())
                    })?;
                    request.required_compat = Some(CompatRule::API);
                    break Changes::RequestPackage(RequestPackage::new(request));
                }
                RequestEnum::Request(request) => match request {
                    Request::Pkg(request) => {
                        break Changes::RequestPackage(RequestPackage::new(request))
                    }
                    Request::Var(request) => break Changes::RequestVar(RequestVar::new(request)),
                },
            }
        };
        self.initial_state_builders.push(request);
        Ok(())
    }

    pub fn get_initial_state(&self) -> State {
        let mut state = State::default();
        for change in self.initial_state_builders.iter() {
            state = change.apply(&state)
        }
        state
    }

    pub fn reset(&mut self) {
        self.repos.clear();
        self.initial_state_builders.clear();
        self.validators = validation::default_validators();
    }

    /// If true, only solve pre-built binary packages.
    ///
    /// When false, the solver may return packages where the build is not set.
    /// These packages are known to have a source package available, and the requested
    /// options are valid for a new build of that source package.
    /// These packages are not actually built as part of the solver process but their
    /// build environments are fully resolved and dependencies included
    pub fn set_binary_only(&mut self, binary_only: bool) {
        let has_binary_only = self
            .validators
            .iter()
            .find_map(|v| match v {
                Validators::BinaryOnly(_) => Some(true),
                _ => None,
            })
            .unwrap_or(false);
        if !(has_binary_only ^ binary_only) {
            return;
        }
        if binary_only {
            // Add BinaryOnly validator because it was missing.
            self.validators
                .insert(0, Validators::BinaryOnly(BinaryOnlyValidator {}))
        } else {
            // Remove all BinaryOnly validators because one was found.
            self.validators = self
                .validators
                .iter()
                .filter(|v| !matches!(v, Validators::BinaryOnly(_)))
                .copied()
                .collect();
        }
    }

    pub fn solve(&mut self) -> PyResult<Solution> {
        let solve_graph = Arc::new(RwLock::new(Graph::new()));
        self.last_graph = solve_graph.clone();

        let mut history = Vec::<Arc<RwLock<Node>>>::new();
        let mut current_node: Option<Arc<RwLock<Node>>> = None;
        let mut decision = Some(Decision::new(self.initial_state_builders.clone()));

        while decision.is_some()
            && (current_node.is_none()
                || !current_node
                    .as_ref()
                    .map(|n| Arc::ptr_eq(&n.read().unwrap().state, &DEAD_STATE))
                    .unwrap_or_default())
        {
            // The python code would `yield (current_node, decision)` here,
            // capturing the yielded value into SolverRuntime.solution.

            current_node = Some({
                let mut sg = solve_graph.write().unwrap();
                let root_id = sg.root.read().unwrap().id();
                match sg.add_branch(
                    current_node
                        .as_ref()
                        .map(|n| n.read().unwrap().id())
                        .unwrap_or(root_id),
                    &decision.unwrap(),
                ) {
                    Ok(cn) => cn,
                    Err(GraphError::RecursionError(msg)) => {
                        match history.pop() {
                            Some(n) => {
                                let n_lock = n.read().unwrap();
                                decision = Some(
                                    Changes::StepBack(StepBack::new(
                                        &msg.to_string(),
                                        &n_lock.state,
                                    ))
                                    .as_decision(),
                                )
                            }
                            None => {
                                decision = Some(
                                    Changes::StepBack(StepBack::new(&msg.to_string(), &DEAD_STATE))
                                        .as_decision(),
                                )
                            }
                        }
                        continue;
                    }
                }
            });
            let current_node = current_node
                .as_ref()
                .expect("current_node always `is_some` here");
            let mut current_node_lock = current_node.write().unwrap();
            decision = self
                .step_state(&mut current_node_lock)
                .map_err(|err| -> PyErr { err.into() })?;
            history.push(current_node.clone());
        }

        let current_node = current_node.expect("current_node always `is_some` here");
        let current_node_lock = current_node.read().unwrap();

        let is_dead = current_node_lock.state.id()
            == solve_graph.read().unwrap().root.read().unwrap().state.id()
            || Arc::ptr_eq(&current_node_lock.state, &DEAD_STATE);
        let is_empty = self.get_initial_state().get_pkg_requests().is_empty();
        if is_dead && !is_empty {
            Err(SolverFailedError::new_err(
                (*solve_graph).read().unwrap().clone(),
            ))
        } else {
            Ok(current_node_lock.state.as_solution()?)
        }
    }

    /// Adds requests for all build requirements and solves
    pub fn solve_build_environment(&mut self, spec: &api::Spec) -> crate::Result<Solution> {
        let state = self.get_initial_state();

        let build_options = spec.resolve_all_options(&state.get_option_map());
        for option in &spec.build.options {
            if let api::Opt::Pkg(option) = option {
                let given = build_options.get(&option.pkg);
                let request = option.to_request(given.cloned())?;
                self.add_request(RequestEnum::Request(request))?;
            }
        }

        Ok(self.solve()?)
    }

    pub fn update_options(&mut self, options: OptionMap) {
        self.initial_state_builders
            .push(Changes::SetOptions(graph::SetOptions::new(options)))
    }

    fn validate(&self, node: &State, spec: &api::Spec) -> crate::Result<api::Compatibility> {
        for validator in &self.validators {
            let compat = validator.validate(node, spec)?;
            if !&compat {
                return Ok(compat);
            }
        }
        Ok(api::Compatibility::Compatible)
    }
}
