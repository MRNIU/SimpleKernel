/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#pragma once

#include <etl/singleton.h>

#include "pl011/pl011_driver.hpp"

using Pl011Singleton = etl::singleton<pl011::Pl011Device>;
