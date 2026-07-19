use anyhow::Result;

use crate::{
    cli::{DoctorArgs, UsageProfile},
    providers::nvidia::diagnostics::{self, DoctorProfile},
    ui::output,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoctorOutcome {
    Healthy,
    ErrorsFound,
}

pub fn run(args: DoctorArgs) -> Result<DoctorOutcome> {
    let diagnostics = diagnostics::detect(match args.profile {
        UsageProfile::ModelTraining => DoctorProfile::ModelTraining,
        UsageProfile::CudaDevelopment => DoctorProfile::CudaDevelopment,
    })?;
    let errors = diagnostics.has_errors();
    output::diagnostics(&diagnostics);
    Ok(if errors {
        DoctorOutcome::ErrorsFound
    } else {
        DoctorOutcome::Healthy
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        device::GpuVendor,
        environment::{
            DiagnosticCheck, DiagnosticId, DiagnosticSection, DiagnosticStatus, Diagnostics,
            FixPlan,
        },
    };

    #[test]
    fn error_status_is_distinct_from_success() {
        let diagnostics = Diagnostics {
            vendor: GpuVendor::Nvidia,
            checks: vec![DiagnosticCheck {
                id: DiagnosticId::NvidiaGpu,
                section: DiagnosticSection::Hardware,
                name: "GPU".into(),
                status: DiagnosticStatus::Error,
                evidence: vec![],
                problem: None,
                dependencies: vec![],
                recommended_fixes: vec![],
            }],
            fix_plan: FixPlan::default(),
        };
        assert!(diagnostics.has_errors());
        assert_eq!(outcome_for(&[diagnostics]), DoctorOutcome::ErrorsFound);
        assert_eq!(outcome_for(&[]), DoctorOutcome::Healthy);
    }

    fn outcome_for(diagnostics: &[Diagnostics]) -> DoctorOutcome {
        if diagnostics.iter().any(Diagnostics::has_errors) {
            DoctorOutcome::ErrorsFound
        } else {
            DoctorOutcome::Healthy
        }
    }
}
