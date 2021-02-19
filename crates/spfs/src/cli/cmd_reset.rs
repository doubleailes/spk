use spfs::prelude::*;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct CmdReset {
    #[structopt(
        long = "edit",
        short = "e",
        about = "mount the /spfs filesystem in edit mode (true if REF is empty or not given)"
    )]
    edit: bool,
    #[structopt(
        long = "ref",
        short = "r",
        about = "The tag or id of the desired runtime, or the current runtime if not given. \
                Use '-' or an empty string to request an empty environment. Only valid \
                if no paths are given"
    )]
    reference: Option<String>,
    #[structopt(
        min_values = 0,
        about = "Paths under /spfs to reset, or all paths if none given"
    )]
    paths: Vec<String>,
}

impl CmdReset {
    pub async fn run(&mut self, config: &spfs::Config) -> spfs::Result<()> {
        let mut runtime = spfs::active_runtime()?;
        let repo = config.get_repository()?;
        if let Some(reference) = &self.reference {
            runtime.reset::<&str>(&[])?;
            runtime.reset_stack()?;
            match reference.as_str() {
                "" | "-" => self.edit = true,
                _ => {
                    let env_spec = spfs::tracking::parse_env_spec(reference)?;
                    for target in env_spec.iter() {
                        let obj = repo.read_ref(target.to_string().as_ref())?;
                        runtime.push_digest(&obj.digest()?)?;
                    }
                }
            }
        } else {
            let paths = strip_spfs_prefix(&self.paths);
            runtime.reset(paths.as_slice())?;
        }

        if self.edit {
            runtime.set_editable(true)?;
        }

        spfs::remount_runtime(&runtime)
    }
}

fn strip_spfs_prefix(paths: &Vec<String>) -> Vec<String> {
    paths
        .into_iter()
        .map(|path| {
            path.strip_prefix("/spfs")
                .unwrap_or(path.as_ref())
                .to_owned()
        })
        .collect()
}
