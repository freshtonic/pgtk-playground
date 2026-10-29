INSERT INTO departments (name) VALUES
    ('Engineering'), ('Sales'), ('Finance'), ('People');

INSERT INTO employees (name, email, department_id, manager_id, salary, hired_on, active) VALUES
    ('Ada Moreau',     'ada@example.com',     1, NULL, 185000, '2019-03-04', true),
    ('Ben Okafor',     'ben@example.com',     1, 1,    142000, '2020-07-13', true),
    ('Chen Wei',       'chen@example.com',    1, 1,    138500, '2021-01-11', true),
    ('Dana Silva',     'dana@example.com',    1, 2,    112000, '2022-09-26', true),
    ('Eli Novak',      'eli@example.com',     1, 2,    109000, '2023-02-06', true),
    ('Fatima Rahman',  'fatima@example.com',  1, 3,    121000, '2021-11-15', false),
    ('Gus Lindqvist',  'gus@example.com',     2, NULL, 160000, '2018-05-21', true),
    ('Hana Sato',      'hana@example.com',    2, 7,     98000, '2022-03-14', true),
    ('Ivan Petrov',    'ivan@example.com',    2, 7,     91000, '2023-08-01', true),
    ('Jo Mensah',      'jo@example.com',      2, 7,     87500, '2024-01-08', true),
    ('Kiri Walker',    'kiri@example.com',    3, NULL, 150000, '2017-10-02', true),
    ('Luis Ortega',    'luis@example.com',    3, 11,   104000, '2020-04-20', true),
    ('Mei Tanaka',     'mei@example.com',     3, 11,    99000, '2022-06-27', true),
    ('Noor Haddad',    'noor@example.com',    4, NULL, 135000, '2019-09-09', true),
    ('Omar Farouk',    'omar@example.com',    4, 14,    82000, '2023-05-15', true),
    ('Priya Nair',     'priya@example.com',   4, 14,    79500, '2024-03-18', true),
    ('Quinn Byrne',    NULL,                  1, 3,     95000, '2025-02-03', true),
    ('Rosa Delgado',   'rosa@example.com',    NULL, NULL, 70000, '2025-06-30', true);

INSERT INTO projects (name, department_id) VALUES
    ('Query planner', 1), ('Billing revamp', 3), ('Onboarding', 4),
    ('Pipeline review', 2), ('Type checker', 1);
