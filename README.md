# VIPayment PHP SDK

[![Latest Stable Version](https://poser.pugx.org/vipayment/vipayment-php/v/stable)](https://packagist.org/packages/vipayment/vipayment-php)
[![Total Downloads](https://poser.pugx.org/vipayment/vipayment-php/downloads)](https://packagist.org/packages/vipayment/vipayment-php)
[![License](https://poser.pugx.org/vipayment/vipayment-php/license)](https://packagist.org/packages/vipayment/vipayment-php)

Official PHP SDK for VIPayment API integration.

## Installation

Install via Composer:

```bash
composer require vipayment/vipayment-php
```

## Usage

### Initialization

```php
require 'vendor/autoload.php';

use Vipayment\Vipayment;

$apiId = 'your_api_id';
$apiKey = 'your_api_key';

$vipayment = new Vipayment($apiId, $apiKey);
```

### Get Profile

```php
$profile = $vipayment->profile();
print_r($profile);
```

### Prepaid Services

#### Order Prepaid

```php
// Example: Order PLN Token
$result = $vipayment->orderPrepaid('PLN5', '3335211111133');
```

#### Check Order Status

```php
$status = $vipayment->statusOrderPrepaid('TRX123456789');
```

#### Get Services List

```php
// Get all services
$services = $vipayment->servicePrepaid();

// Filter by type
$services = $vipayment->servicePrepaid('type', 'pulsa-reguler');
```

### Game & Streaming Services

#### Order Game

```php
// Example: Topup Free Fire
$result = $vipayment->orderGame('FF5', '1234567890');
```

#### Check Game Order Status

```php
$status = $vipayment->statusOrderGame('TRX987654321');
```

#### Get Game Services List

```php
// Get all game services
$services = $vipayment->serviceGame();

// Filter by brand
$services = $vipayment->serviceGame('mobile-legends', 'available');
```

## License

MIT
