#[cfg(test)]
mod test {
    use crate::{
        algebra::{
            evaluation_domain::BatchEvaluationDomain,
            fft::fft_assign,
            lagrange::{all_n_lagrange_coefficients, lagrange_coefficients, FFT_THRESH},
            polynomials::poly_eval,
        },
        utils::random::{random_scalar, random_scalars},
    };
    use blstrs::Scalar;
    use ff::Field;
    use rand::{seq::IteratorRandom, thread_rng};
    use std::ops::Mul;

    #[test]
    fn test_lagrange() {
        let mut rng = thread_rng();

        for n in 1..=FFT_THRESH * 2 {
            for t in 1..=n {
                // println!("t = {t}, n = {n}");
                let deg = t - 1; // the degree of the polynomial

                // pick a random $f(X)$
                let f = random_scalars(deg + 1, &mut rng);

                // give shares to all the $n$ players: i.e., evals[i] = f(\omega^i)
                let batch_dom = BatchEvaluationDomain::new(n);
                let mut evals = f.clone();
                fft_assign(&mut evals, &batch_dom.get_subdomain(n));

                // try to reconstruct $f(0)$ from a random subset of t shares
                let mut players: Vec<usize> = (0..n)
                    .choose_multiple(&mut rng, t)
                    .into_iter()
                    .collect::<Vec<usize>>();

                players.sort();

                let lagr = lagrange_coefficients(&batch_dom, players.as_slice(), &Scalar::ZERO);
                // println!("lagr: {:?}", lagr);

                let mut s = Scalar::ZERO;
                for i in 0..t {
                    s += lagr[i].mul(evals[players[i]]);
                }

                // println!("s   : {s}");
                // println!("f[0]: {}", f[0]);

                assert_eq!(s, f[0]);
            }
        }
    }

    #[test]
    #[allow(non_snake_case)]
    fn test_all_N_lagrange() {
        let mut rng = thread_rng();

        let mut Ns = vec![2];
        while *Ns.last().unwrap() < FFT_THRESH {
            Ns.push(Ns.last().unwrap() * 2);
        }

        for N in Ns {
            // the degree of the polynomial is N - 1

            // pick a random $f(X)$
            let f = random_scalars(N, &mut rng);

            // give shares to all the $n$ players: i.e., evals[i] = f(\omega^i)
            let batch_dom = BatchEvaluationDomain::new(N);
            let mut evals = f.clone();
            fft_assign(&mut evals, &batch_dom.get_subdomain(N));

            // try to reconstruct $f(\alpha)$ from all $N$ shares
            let alpha = random_scalar(&mut rng);
            let lagr1 = all_n_lagrange_coefficients(&batch_dom, &alpha);

            let all = (0..N).collect::<Vec<usize>>();
            let lagr2 = lagrange_coefficients(&batch_dom, all.as_slice(), &alpha);
            assert_eq!(lagr1, lagr2);

            let mut f_of_alpha = Scalar::ZERO;
            for i in 0..N {
                f_of_alpha += lagr1[i].mul(evals[i]);
            }

            let f_of_alpha_eval = poly_eval(&f, &alpha);
            // println!("f(\\alpha) interpolated: {f_of_alpha}");
            // println!("f(\\alpha) evaluated   : {f_of_alpha_eval}");

            assert_eq!(f_of_alpha, f_of_alpha_eval);
        }
    }
}