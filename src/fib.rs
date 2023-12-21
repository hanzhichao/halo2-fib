use halo2_proofs::circuit::{Value, Layouter, AssignedCell, SimpleFloorPlanner};
use halo2_proofs::poly::Rotation;
use halo2_proofs::{plonk::*};
use halo2_proofs::arithmetic::Field;
use halo2_proofs::dev::MockProver;
use halo2_proofs::pasta::Fp;
use plotters::prelude::WHITE;

#[derive(Clone, Debug, Copy)]
struct FibConfig {
    selector: Selector,
    a: Column<Advice>,
    b: Column<Advice>,
    target: Column<Instance>,
}

struct FibChip {
    config: FibConfig
}

impl FibChip {
    fn configure<F: Field>(meta: &mut ConstraintSystem<F>) -> FibConfig {
        let selector = meta.selector();
        let a = meta.advice_column();
        let b = meta.advice_column();
        let target = meta.instance_column();

        meta.enable_equality(a);
        meta.enable_equality(b);
        meta.enable_equality(target);

        meta.create_gate("斐波那契(相加)", |meta| {
            let selector = meta.query_selector(selector);
            let num_a = meta.query_advice(a, Rotation::cur());
            let num_b = meta.query_advice(b, Rotation::cur());
            let next_b = meta.query_advice(b, Rotation::next());
            vec![
                ("a + b = next_b", selector * (num_a + num_b - next_b)),
            ]
        });
        FibConfig { selector, a, b, target }
    }

    fn assign_first_row<F: Field>(&self, mut layouter: impl Layouter<F>, a: Value<F>, b: Value<F>) -> Result<(AssignedCell<F, F>, AssignedCell<F, F>), Error> {
        layouter.assign_region(|| "填写第一行", |mut region| {
            self.config.selector.enable(&mut region, 0)?;
            region.assign_advice(|| "加载a", self.config.a,  0, || a).expect("加载a失败");
            let cur_b = region.assign_advice(|| "加载b", self.config.b,  0, || b).expect("加载b失败");
            let next_b = region.assign_advice(|| "计算当前c", self.config.b,  1, || a+b).expect("填写下一行b失败");
            Ok((cur_b, next_b))
        })
    }

    fn assign_next_row<F: Field>(&self, mut layouter: impl Layouter<F>, pre_b: &AssignedCell<F,F>, pre_c: &AssignedCell<F, F>) -> Result<(AssignedCell<F, F>, AssignedCell<F, F>), Error> {
        layouter.assign_region(|| "填写下一行", |mut region| {
            self.config.selector.enable(&mut region, 0)?;
            let cur_a = pre_b.copy_advice(|| "拷贝上一行b到当前a", &mut region, self.config.a, 0).expect("拷贝到a失败");
            let cur_b = pre_c.copy_advice(|| "拷贝上一行c到当前b", &mut region, self.config.b, 0).expect("拷贝到b失败");
            let sum = cur_a.value_field().evaluate() + cur_b.value_field().evaluate();
            let next_b = region.assign_advice(|| "计算当前c", self.config.b, 1, || sum).expect("填写下一行b失败");
            Ok((cur_b, next_b))
        })
    }

    fn expose_public<F:Field>( &self,  mut layouter: impl Layouter<F>, cell: &AssignedCell<F,F>, row: usize ) -> Result<(), Error> {
        layouter.constrain_instance(cell.cell(), self.config.target, row)
    }
}


#[derive(Default)]
struct FibCircuit<F: Field> {
    a: Value<F>, // 初始a=1
    b: Value<F>, // 初始b=1
}

impl<F: Field> Circuit<F> for FibCircuit<F> {
    type Config = FibConfig;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {Self::default()}

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {FibChip::configure(meta) }

    fn synthesize(&self, config: Self::Config, mut layouter: impl Layouter<F>) -> Result<(), Error> {
        let fib = FibChip { config };
        // 初始化第一行
        let (mut a, mut b) = fib.assign_first_row(layouter.namespace(||"填写第一行"), self.a, self.b).expect("填写第一行失败");
        // 循环填写下一行
        for _i in 3..10 {
            let (next_a, next_b) = fib.assign_next_row(layouter.namespace(||"填写下一行"), &a, &b).expect("填写下一行失败");
            a = next_a;
            b = next_b;
        }
        // 暴露结果
        fib.expose_public(layouter, &b, 0)?;
        Ok(())
    }
}

#[test]
fn test_fib() {
    let circuit = FibCircuit {a: Value::known(Fp::one()),b: Value::known(Fp::one())};
    let target = Fp::from(55);
    let public_input = vec![target];
    let prover = MockProver::run(5, &circuit, vec![public_input]).unwrap();
    prover.assert_satisfied();
}

#[cfg(feature = "dev")]
#[test]
fn print_fib() {
    use plotters::prelude::*;
    use halo2_proofs::pasta::Fp;

    let root = BitMapBackend::new("fib-layout.png", (1024, 3096)).into_drawing_area();
    root.fill(&WHITE).unwrap();
    let root = root.titled("Fib Layout", ("sans-serif", 60)).unwrap();

    let circuit = FibCircuit {
        a: Value::known(Fp::one()),
        b: Value::known(Fp::one()),
    };
    halo2_proofs::dev::CircuitLayout::default()
        .render(5, &circuit, &root)
        .unwrap();

    let dot_string = halo2_proofs::dev::circuit_dot_graph(&circuit);
    print!("{}", dot_string);
}