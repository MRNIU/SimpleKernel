/**
 * @copyright Copyright The SimpleKernel Contributors
 */

/// @brief Minimal math stubs for freestanding environment.
///
/// ETL's etl::format instantiates floating-point formatting templates that
/// reference pow, modf, etc. from <math.h>.  The kernel never actually formats
/// floating-point values, so these paths are unreachable at runtime.  We
/// provide stub definitions here solely to satisfy the linker.
///
/// @warning These stubs return 0.  Do NOT rely on them for real math.

double pow(double base, double exp) {
  (void)base;
  (void)exp;
  return 0.0;
}

float powf(float base, float exp) {
  (void)base;
  (void)exp;
  return 0.0f;
}

long double powl(long double base, long double exp) {
  (void)base;
  (void)exp;
  return 0.0L;
}

double modf(double x, double* iptr) {
  if (iptr) {
    *iptr = x;
  }
  return 0.0;
}

float modff(float x, float* iptr) {
  if (iptr) {
    *iptr = x;
  }
  return 0.0f;
}

long double modfl(long double x, long double* iptr) {
  if (iptr) {
    *iptr = x;
  }
  return 0.0L;
}
