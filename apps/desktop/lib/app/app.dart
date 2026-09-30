import 'package:flutter/material.dart';
import 'package:v2rayn_desktop/features/profiles/profiles_page.dart';

class V2rayNRApp extends StatelessWidget {
  const V2rayNRApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'v2rayN-R (T01)',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        useMaterial3: true,
        colorSchemeSeed: const Color(0xFF1565C0),
      ),
      home: const ProfilesPage(),
    );
  }
}
